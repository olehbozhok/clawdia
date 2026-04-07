"""Campaign Document MCP Server.

Exposes doc_* tools for managing campaign documents:
create, add statements, set verdicts, add media, write content, assemble, publish.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path

from mcp.server.fastmcp import FastMCP

from store import CampaignStatus, CampaignStore, Verdict


def _parse_storage_dir() -> Path | None:
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("--storage-dir", default=None)
    args, _ = parser.parse_known_args()
    raw = args.storage_dir or os.environ.get("CAMPAIGN_DOC_STORAGE_DIR")
    if raw:
        return Path(raw)
    return None


mcp = FastMCP("campaign-doc")
store = CampaignStore(storage_dir=_parse_storage_dir())


@mcp.tool()
def doc_create(topic: str) -> str:
    """Create a new campaign document.

    Args:
        topic: The campaign topic, e.g. "Impact of bottom trawling on ocean ecosystems"
    """
    campaign = store.create(topic)
    return json.dumps({"campaign_id": campaign.id, "status": campaign.status.value})


@mcp.tool()
def doc_add_statement(
    campaign_id: str,
    text: str,
    source_url: str,
) -> str:
    """Add a statement with its citation to a campaign.

    The source domain is extracted automatically from the URL.

    Args:
        campaign_id: Campaign ID (e.g. "camp-1")
        text: The statement text
        source_url: URL of the source article (e.g. "https://fisheries.noaa.gov/article/123")
    """
    statement = store.add_statement(campaign_id, text, source_url)
    return json.dumps({"statement_id": statement.id})


@mcp.tool()
def doc_set_verdict(
    campaign_id: str,
    statement_id: str,
    verdict: str,
    reason: str,
) -> str:
    """Set a verification verdict on a statement.

    Args:
        campaign_id: Campaign ID (e.g. "camp-1")
        statement_id: Statement ID (e.g. "s1")
        verdict: One of: "verified", "rejected", "needs_revision"
        reason: Explanation for the verdict
    """
    v = Verdict(verdict)
    statement = store.set_verdict(campaign_id, statement_id, v, reason)
    return json.dumps({"statement_id": statement.id, "verdict": v.value})


@mcp.tool()
def doc_add_media(
    campaign_id: str,
    media_type: str,
    ref: str,
    description: str,
) -> str:
    """Add a media reference to a campaign.

    Args:
        campaign_id: Campaign ID (e.g. "camp-1")
        media_type: Type of media: "image" or "video"
        ref: URL or file path of the media
        description: Description of the media content
    """
    media = store.add_media(campaign_id, media_type, ref, description)
    return json.dumps({"media_id": media.id})


@mcp.tool()
def doc_write_content(
    campaign_id: str,
    headline: str,
    body: str,
    instagram_caption: str,
    call_to_action: str,
) -> str:
    """Write campaign text content.

    Args:
        campaign_id: Campaign ID (e.g. "camp-1")
        headline: Campaign headline
        body: Main campaign body text
        instagram_caption: Instagram post caption
        call_to_action: Call to action text
    """
    content = store.write_content(
        campaign_id, headline, body, instagram_caption, call_to_action,
    )
    return json.dumps({"content_id": content.id})


@mcp.tool()
def doc_assemble(campaign_id: str) -> str:
    """Assemble a campaign package from verified statements only.

    Includes only verified statements. Rejected and needs_revision statements are excluded.

    Args:
        campaign_id: Campaign ID (e.g. "camp-1")
    """
    package = store.assemble(campaign_id)
    return json.dumps({
        "package_id": package.id,
        "verified_statements": package.verified_statements,
        "included_statements": package.included_statements,
        "excluded_statements": package.excluded_statements,
        "has_content": package.has_content,
        "media_count": package.media_count,
        "ready_to_publish": package.ready_to_publish,
    })


@mcp.tool()
def doc_list_statements(
    campaign_id: str,
    verdict: str | None = None,
) -> str:
    """List statements in a campaign with their IDs, text, source, and verdict.

    Use this to discover statement IDs before calling doc_set_verdict or to check
    which statements still need verification.

    Args:
        campaign_id: Campaign ID (e.g. "camp-1")
        verdict: Optional filter: "verified", "rejected", "needs_revision", or "unverified"
    """
    return json.dumps(store.list_statements(campaign_id, verdict_filter=verdict))


@mcp.tool()
def doc_list(status: str | None = None) -> str:
    """List all campaigns with their ID, topic, status, and statement count.

    Args:
        status: Optional filter by status: "created", "in_progress", "assembled", "draft_saved", "published"
    """
    filter_status = CampaignStatus(status) if status else None
    return json.dumps(store.list_campaigns(status=filter_status))


@mcp.tool()
def doc_status(campaign_id: str) -> str:
    """Get lightweight campaign metadata: counts and readiness flags. No texts or URLs.

    Use this to check progress without spending tokens on full content.

    Args:
        campaign_id: Campaign ID (e.g. "camp-1")
    """
    return json.dumps(store.status(campaign_id))


@mcp.tool()
def doc_get(campaign_id: str) -> str:
    """Get the full campaign state including all texts, URLs, and metadata.

    Args:
        campaign_id: Campaign ID (e.g. "camp-1")
    """
    return json.dumps(store.full_state(campaign_id))


@mcp.tool()
def doc_publish_draft(campaign_id: str) -> str:
    """Save the assembled campaign as a draft. No approval required.

    Requires doc_assemble to be called first.

    Args:
        campaign_id: Campaign ID (e.g. "camp-1")
    """
    return json.dumps(store.publish_draft(campaign_id))


@mcp.tool()
def doc_publish_live(campaign_id: str) -> str:
    """Publish the campaign live. Requires assembled package with verified statements and content.

    Requires doc_assemble to be called first.
    Future: will require human approval via id_token and userinfo_token.

    Args:
        campaign_id: Campaign ID (e.g. "camp-1")
    """
    return json.dumps(store.publish_live(campaign_id))


if __name__ == "__main__":
    mcp.run(transport="stdio")
