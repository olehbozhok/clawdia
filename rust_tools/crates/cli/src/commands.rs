use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use rig::completion::Prompt;
use rig::providers::deepseek;
use runtime::agents;
use runtime::authz_hook::AuthzBackend;
use runtime::cedar_authz::CedarAuthz;
use runtime::policy_prompt;
use runtime::runtime::build_runtime;
use runtime::sessions::Principal;

use crate::cli_args::{AuthzBackendChoice, ChatArgs};
use crate::tui::keymap::{self, Action};
use crate::tui::log_layer::LogLine;
use crate::tui::tracing_init;
use crate::tui::{App, AppEvent, Pane};

pub async fn chat(args: &ChatArgs) -> anyhow::Result<()> {
    let (log_tx, mut log_rx) = tokio::sync::mpsc::unbounded_channel::<LogLine>();
    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();

    let _appender_guard = tracing_init::init_tracing(log_tx.clone(), &args.log_dir)?;

    let config = agents::load_config(&args.agents_config)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let (servers, running_services) = runtime::mcp::connect_all(&args.mcp_config)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let cedar = match args.authz_backend {
        AuthzBackendChoice::Cedarling => {
            let cedar = CedarAuthz::from_directory(&args.policy_store).await.map_err(
                |e| {
                    anyhow::anyhow!(
                        "Cedarling backend selected but failed to load policy store \
                     at {}: {e}. Re-run with --authz-backend yaml only for \
                     development/debugging.",
                        args.policy_store.display(),
                    )
                },
            )?;
            tracing::info!("Cedar policy engine loaded from {}", args.policy_store.display());
            Some(Arc::new(cedar))
        }
        AuthzBackendChoice::Yaml => {
            tracing::warn!(
                "YAML authorization backend selected — Cedar policies are NOT \
                 enforced. This mode is for development/debugging only."
            );
            None
        }
    };

    let backend = match &cedar {
        Some(c) => AuthzBackend::Cedar(c.clone()),
        None => AuthzBackend::Yaml,
    };

    let _agent_permissions = if args.policy_store.exists() {
        policy_prompt::load_permissions_from_policies(&args.policy_store).unwrap_or_default()
    } else {
        HashMap::new()
    };

    let (runtime, cancel, _bg_tasks) = build_runtime(
        &args.approver_hmac_key,
        &args.local_key_id,
        &args.local_roles,
        backend,
    );

    let sid = runtime
        .session_store
        .create_root(
            "orchestrator".into(),
            Principal("anon".into()),
            None,
        )
        .await?;

    let client = deepseek::Client::new(&args.api_key)?;
    let orch = agents::build_orchestrator(
        &runtime,
        sid.clone(),
        &client,
        &args.model,
        &config.orchestrator,
        &servers,
    )?;

    let glue = crate::tui::runtime_glue::RuntimeGlue {
        gateway: runtime.gateway.clone(),
        sessions: runtime.session_store.clone(),
        inbox: runtime.inbox.clone(),
        notifications: runtime.notification_store.clone(),
        hmac_key: Arc::new(runtime.hmac_key.clone()),
        local_key_id: runtime.local_key_id.clone(),
        local_roles: runtime.local_roles.clone(),
        root_session: sid,
    };

    let _bg = glue.clone().spawn(event_tx.clone(), cancel.clone());

    // Forward log channel to event channel
    let log_event_tx = event_tx.clone();
    tokio::spawn(async move {
        while let Some(line) = log_rx.recv().await {
            let _ = log_event_tx.send(AppEvent::Log(line));
        }
    });

    let mut terminal = ratatui::init();
    let mut app = App::new();
    let mut ticker = tokio::time::interval(std::time::Duration::from_millis(250));

    let crossterm_tx = event_tx.clone();
    tokio::spawn(async move {
        let mut reader = crossterm::event::EventStream::new();
        loop {
            use futures_util::StreamExt;
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    let _ = crossterm_tx.send(AppEvent::Quit);
                    break;
                }
                event = reader.next() => {
                    match event {
                        Some(Ok(crossterm::event::Event::Key(key))) => {
                            let _ = crossterm_tx.send(AppEvent::Key(key));
                        }
                        Some(Err(_)) | None => break,
                        _ => {}
                    }
                }
            }
        }
    });

    let result: anyhow::Result<()> = loop {
        tokio::select! {
            _ = ticker.tick() => {
                app.reduce(AppEvent::Tick(std::time::Instant::now()));
            }
            Some(event) = event_rx.recv() => {
                match event {
                    AppEvent::Key(key) => {
                        let action = keymap::dispatch(app.focus, key);
                        match action {
                            Action::Quit => {
                                app.reduce(AppEvent::Quit);
                                break Ok(());
                            }
                            Action::FocusNext => app.reduce(AppEvent::FocusNext),
                            Action::SelectNext => {
                                if app.focus == Pane::Approvals {
                                    app.reduce(AppEvent::Approval(
                                        crate::tui::approvals_pane::ApprovalEvent::SelectNext,
                                    ));
                                } else if app.focus == Pane::Log {
                                    app.log.scroll_up();
                                }
                            }
                            Action::SelectPrev => {
                                if app.focus == Pane::Approvals {
                                    app.reduce(AppEvent::Approval(
                                        crate::tui::approvals_pane::ApprovalEvent::SelectPrev,
                                    ));
                                } else if app.focus == Pane::Log {
                                    app.log.scroll_down();
                                }
                            }
                            Action::InputChar(c) => {
                                app.chat.input.push(c);
                            }
                            Action::InputBackspace => {
                                app.chat.input.pop();
                            }
                            Action::SubmitChat(_) => {
                                let text = app.chat.input.clone();
                                if !text.is_empty() {
                                    app.chat.history.push(
                                        crate::tui::chat_pane::ChatMsg::User(text.clone()),
                                    );
                                    app.chat.input.clear();
                                    match orch.agent.prompt(&text).await {
                                        Ok(response) => {
                                            app.chat.history.push(
                                                crate::tui::chat_pane::ChatMsg::Agent {
                                                    label: "orchestrator".into(),
                                                    text: response,
                                                },
                                            );
                                        }
                                        Err(e) => {
                                            tracing::error!("agent error: {e}");
                                        }
                                    }
                                }
                            }
                            Action::ApprovalDecide(choice) => {
                                if let Some(id) = app.approvals.selected_id() {
                                    let tid = runtime::approvals::types::TicketId(id.to_string());
                                    let _ = glue.approve(&tid, choice).await;
                                }
                            }
                            _ => {}
                        }
                    }
                    AppEvent::Quit => {
                        app.reduce(AppEvent::Quit);
                        break Ok(());
                    }
                    other => {
                        app.reduce(other);
                    }
                }
            }
        }

        if app.quitting {
            break Ok(());
        }

        terminal
            .draw(|f| {
                let area = f.area();
                let main_chunks = ratatui::layout::Layout::default()
                    .direction(ratatui::layout::Direction::Vertical)
                    .constraints([
                        ratatui::layout::Constraint::Percentage(70),
                        ratatui::layout::Constraint::Percentage(30),
                    ])
                    .split(area);

                let top_chunks = ratatui::layout::Layout::default()
                    .direction(ratatui::layout::Direction::Horizontal)
                    .constraints([
                        ratatui::layout::Constraint::Percentage(60),
                        ratatui::layout::Constraint::Percentage(40),
                    ])
                    .split(main_chunks[0]);

                crate::tui::chat_pane::render(
                    f,
                    top_chunks[0],
                    &app.chat,
                    app.focus == Pane::Chat,
                );
                crate::tui::approvals_pane::render(
                    f,
                    top_chunks[1],
                    &app.approvals,
                    app.focus == Pane::Approvals,
                );
                crate::tui::log_pane::render(
                    f,
                    main_chunks[1],
                    &app.log,
                    app.focus == Pane::Log,
                );
            })
            .ok();
    };

    glue.shutdown().await;
    cancel.cancel();
    for svc in running_services {
        let _ = svc.cancel().await;
    }
    ratatui::restore();
    result
}

pub async fn tools(mcp_config: &Path, json: bool) -> anyhow::Result<()> {
    let (servers, running_services) = runtime::mcp::connect_all(mcp_config)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    if json {
        let mut output = serde_json::Map::new();
        for server in &servers {
            let tools: Vec<serde_json::Value> = server
                .tools
                .iter()
                .map(|t| serde_json::to_value(t).unwrap_or_default())
                .collect();
            output.insert(server.name.clone(), serde_json::Value::Array(tools));
        }
        print!(
            "{}",
            serde_json::to_string_pretty(&serde_json::Value::Object(output))?
        );
    } else {
        println!("Available tools from MCP servers:\n");
        for server in &servers {
            println!("  [{name}]", name = server.name);
            for tool in &server.tools {
                let desc = tool.description.as_deref().unwrap_or("(no description)");
                println!("    - {name}", name = tool.name);
                println!("      {desc}\n");
            }
        }
        println!("Copy tool names into agents.yaml → permitted_actions to grant access.");
    }

    for svc in running_services {
        svc.cancel().await.map_err(|e| anyhow::anyhow!("{e}"))?;
    }
    Ok(())
}

pub async fn advisor_generate(
    mcp_config: &Path,
    agents_config: &Path,
    output: &Path,
    api_key: &str,
    model: &str,
    policy_store_id: Option<&str>,
    system_entity_id: &str,
) -> anyhow::Result<()> {
    let agents_cfg = runtime::agents::load_config(agents_config).map_err(|e| anyhow::anyhow!("{e}"))?;

    let mut agents = Vec::new();
    for (name, agent) in &agents_cfg.agents {
        agents.push(advisor::AgentSpec {
            name: name.clone(),
            description: agent.description.clone(),
            permitted_tools: agent.permitted_actions.clone(),
        });
    }

    let tools = advisor::discovery::discover(mcp_config)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("Discovered {} tools across MCP servers.", tools.len());

    let client =
        advisor::RigClient::new(api_key, model).map_err(|e| anyhow::anyhow!("{e}"))?;

    let policy_store_id = match policy_store_id {
        Some(id) => id.to_string(),
        None => random_hex_id(),
    };

    let input = advisor::AdvisorInput {
        agents,
        tools,
        policy_store_id,
        system_entity_id: system_entity_id.to_string(),
        domain_hint: None,
    };

    let out = advisor::run(&client, input, output.to_path_buf())
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("Wrote policy store to {}", out.output_dir.display());
    Ok(())
}

fn random_hex_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:024x}", nanos)
}

pub fn agents(agents_config: &Path) -> anyhow::Result<()> {
    let config = agents::load_config(agents_config).map_err(|e| anyhow::anyhow!("{e}"))?;

    println!("Orchestrator:");
    if config.orchestrator.permitted_actions.is_empty() {
        println!("  permitted_actions: (none — all tools denied)");
    } else {
        println!("  permitted_actions:");
        for action in &config.orchestrator.permitted_actions {
            println!("    - {action}");
        }
    }

    println!("\nSub-agents:\n");
    for (key, agent) in &config.agents {
        println!("  {key}:");
        println!("    name: {}", agent.name);
        println!("    description: {}", agent.description);
        if agent.permitted_actions.is_empty() {
            println!("    permitted_actions: (none — all tools denied)");
        } else {
            println!("    permitted_actions:");
            for action in &agent.permitted_actions {
                println!("      - {action}");
            }
        }
        println!();
    }

    Ok(())
}
