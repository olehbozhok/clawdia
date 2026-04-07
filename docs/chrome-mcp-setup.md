# Chrome MCP Server Setup Guide

This guide walks you through setting up the Chrome MCP server for browser automation in Clawdia.

## Prerequisites

| Requirement | Minimum Version | Notes |
|---|---|---|
| Google Chrome (or Chromium) | Latest stable | Must be a **native** install, not snap |
| Node.js | 20.0.0+ | Use `nvm` if you need to upgrade |
| npm | Comes with Node.js | For installing `mcp-chrome-bridge` |

## Installation

### Step 1: Install Google Chrome (native)

> **Important:** The snap version of Chromium does not support the native messaging required by `mcp-chrome-bridge`. If you have the snap version installed, remove it first.

```bash
# Check if you have the snap version
snap list chromium 2>/dev/null && echo "Snap Chromium detected — remove it first"

# Remove snap version (if installed)
sudo snap remove chromium

# Add Google Chrome repository and install
wget -q -O - https://dl-ssl.google.com/linux/linux_signing_key.pub | sudo apt-key add -
sudo sh -c 'echo "deb [arch=amd64] http://dl.google.com/linux/chrome/deb/ stable main" >> /etc/apt/sources.list.d/google-chrome.list'
sudo apt update
sudo apt install google-chrome-stable
```

Verify the installation:

```bash
google-chrome --version
```

### Step 2: Install mcp-chrome-bridge

```bash
npm install -g mcp-chrome-bridge
```

Verify:

```bash
mcp-chrome-bridge --version
```

### Step 3: Download and load the Chrome MCP extension

1. Download the latest release from: https://github.com/hangwin/mcp-chrome/releases
2. Extract the downloaded archive — you'll get a folder with the extension files.
3. Open Chrome and navigate to `chrome://extensions/`
4. Enable **Developer mode** (toggle in the top-right corner).
5. Click **Load unpacked** and select the extracted extension folder.
6. Click the extension icon in the toolbar and verify it shows **"Connected, Service Started"**.

### Step 4: Register Native Messaging host

This step tells Chrome how to communicate with the `mcp-chrome-bridge` process:

```bash
npx mcp-chrome-bridge register --detect
```

## Configuration

The Chrome MCP server is already configured in `rust_tools/config/mcp_servers.yaml`:

```yaml
chrome-mcp:
  transport: http
  url: http://127.0.0.1:12306/mcp
```

No changes needed unless you run the bridge on a different port.

## Running

You need **three terminals** (or use `tmux`/`screen`):

**Terminal 1 — Start Chrome with remote debugging:**

```bash
google-chrome --remote-debugging-port=9222 --incognito
```

**Terminal 2 — Start the MCP bridge:**

```bash
mcp-chrome-bridge
```

Expected output:

```
MCP Server listening on http://127.0.0.1:12306/mcp
```

**Terminal 3 — Run Clawdia:**

```bash
# Your usual command to start the orchestrator
cargo run --release -p runtime
```

The orchestrator will connect to the Chrome MCP server automatically.

## Verification

Run the built-in diagnostics:

```bash
npx mcp-chrome-bridge doctor
```

All checks should show `[OK]`. If any check fails, see Troubleshooting below.

## Troubleshooting

### Extension shows "Service Not Started"

- Make sure `mcp-chrome-bridge` is running (Terminal 2).
- Click the extension icon in Chrome — it should show "Connected, Service Started".
- Try reloading the extension on `chrome://extensions/`.

### Node.js version error

`mcp-chrome-bridge` requires Node.js v20+. Upgrade with `nvm`:

```bash
nvm install 20
nvm use 20
```

Then re-register:

```bash
npx mcp-chrome-bridge register --detect
```

### Chrome installed via snap

The snap sandbox blocks native messaging. Remove it and install the native `.deb` package (see Step 1).

### Bridge starts but tools don't work

- Confirm Chrome was started with `--remote-debugging-port=9222`.
- Check that port 12306 is not already in use: `lsof -i :12306`.
- Restart both Chrome and the bridge.

## Available Tools

Once connected, the Chrome MCP server exposes 20+ tools. Key ones:

| Tool | Description |
|---|---|
| `chrome_navigate` | Navigate to a URL |
| `chrome_screenshot` | Capture a screenshot of the current page |
| `chrome_get_web_content` | Extract page content as text/HTML |
| `search_tabs_content` | Semantic search across open tabs |
| `chrome_fill_or_select` | Fill form fields or select options |
| `chrome_click` | Click elements on the page |
| `chrome_evaluate` | Execute JavaScript in the page context |

For the full list of tools, run the bridge and inspect its `/mcp` endpoint, or check the [mcp-chrome documentation](https://github.com/hangwin/mcp-chrome).
