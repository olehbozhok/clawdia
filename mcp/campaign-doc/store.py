"""In-memory campaign document store with auto-increment IDs."""

from __future__ import annotations

from dataclasses import dataclass, field
from datetime import datetime, timezone
from enum import Enum
from typing import Any


class Verdict(str, Enum):
    VERIFIED = "verified"
    REJECTED = "rejected"
    NEEDS_REVISION = "needs_revision"


class CampaignStatus(str, Enum):
    CREATED = "created"
    IN_PROGRESS = "in_progress"
    ASSEMBLED = "assembled"
    DRAFT_SAVED = "draft_saved"
    PUBLISHED = "published"


@dataclass
class Statement:
    id: str
    text: str
    source_url: str
    source_domain: str
    domain_tier: str
    verdict: Verdict | None = None
    verdict_reason: str | None = None
    created_at: str = field(default_factory=lambda: _now())


@dataclass
class MediaRef:
    id: str
    media_type: str
    ref: str
    description: str
    created_at: str = field(default_factory=lambda: _now())


@dataclass
class CampaignContent:
    id: str
    headline: str
    body: str
    instagram_caption: str
    call_to_action: str
    created_at: str = field(default_factory=lambda: _now())


@dataclass
class CampaignPackage:
    id: str
    included_statements: list[str]
    excluded_statements: list[str]
    verified_statements: int
    has_content: bool
    media_count: int
    ready_to_publish: bool
    assembled_at: str = field(default_factory=lambda: _now())


@dataclass
class Campaign:
    id: str
    topic: str
    status: CampaignStatus
    statements: list[Statement] = field(default_factory=list)
    media: list[MediaRef] = field(default_factory=list)
    content: CampaignContent | None = None
    package: CampaignPackage | None = None
    created_at: str = field(default_factory=lambda: _now())

    # Auto-increment counters
    _statement_counter: int = field(default=0, repr=False)
    _media_counter: int = field(default=0, repr=False)
    _content_counter: int = field(default=0, repr=False)
    _package_counter: int = field(default=0, repr=False)

    def next_statement_id(self) -> str:
        self._statement_counter += 1
        return f"s{self._statement_counter}"

    def next_media_id(self) -> str:
        self._media_counter += 1
        return f"m{self._media_counter}"

    def next_content_id(self) -> str:
        self._content_counter += 1
        return f"t{self._content_counter}"

    def next_package_id(self) -> str:
        self._package_counter += 1
        return f"pkg-{self._package_counter}"


class CampaignStore:
    def __init__(self) -> None:
        self._campaigns: dict[str, Campaign] = {}
        self._counter: int = 0

    def _next_id(self) -> str:
        self._counter += 1
        return f"camp-{self._counter}"

    def create(self, topic: str) -> Campaign:
        campaign = Campaign(
            id=self._next_id(),
            topic=topic,
            status=CampaignStatus.CREATED,
        )
        self._campaigns[campaign.id] = campaign
        return campaign

    def get(self, campaign_id: str) -> Campaign:
        if campaign_id not in self._campaigns:
            raise KeyError(f"Campaign '{campaign_id}' not found")
        return self._campaigns[campaign_id]

    def add_statement(
        self,
        campaign_id: str,
        text: str,
        source_url: str,
        source_domain: str,
        domain_tier: str,
    ) -> Statement:
        campaign = self.get(campaign_id)
        statement = Statement(
            id=campaign.next_statement_id(),
            text=text,
            source_url=source_url,
            source_domain=source_domain,
            domain_tier=domain_tier,
        )
        campaign.statements.append(statement)
        campaign.status = CampaignStatus.IN_PROGRESS
        return statement

    def set_verdict(
        self,
        campaign_id: str,
        statement_id: str,
        verdict: Verdict,
        reason: str,
    ) -> Statement:
        campaign = self.get(campaign_id)
        for statement in campaign.statements:
            if statement.id == statement_id:
                statement.verdict = verdict
                statement.verdict_reason = reason
                return statement
        raise KeyError(f"Statement '{statement_id}' not found in campaign '{campaign_id}'")

    def add_media(
        self,
        campaign_id: str,
        media_type: str,
        ref: str,
        description: str,
    ) -> MediaRef:
        campaign = self.get(campaign_id)
        media = MediaRef(
            id=campaign.next_media_id(),
            media_type=media_type,
            ref=ref,
            description=description,
        )
        campaign.media.append(media)
        campaign.status = CampaignStatus.IN_PROGRESS
        return media

    def write_content(
        self,
        campaign_id: str,
        headline: str,
        body: str,
        instagram_caption: str,
        call_to_action: str,
    ) -> CampaignContent:
        campaign = self.get(campaign_id)
        content = CampaignContent(
            id=campaign.next_content_id(),
            headline=headline,
            body=body,
            instagram_caption=instagram_caption,
            call_to_action=call_to_action,
        )
        campaign.content = content
        campaign.status = CampaignStatus.IN_PROGRESS
        return content

    def assemble(self, campaign_id: str) -> CampaignPackage:
        campaign = self.get(campaign_id)
        included = [c.id for c in campaign.statements if c.verdict == Verdict.VERIFIED]
        excluded = [c.id for c in campaign.statements if c.verdict != Verdict.VERIFIED]

        package = CampaignPackage(
            id=campaign.next_package_id(),
            included_statements=included,
            excluded_statements=excluded,
            verified_statements=len(included),
            has_content=campaign.content is not None,
            media_count=len(campaign.media),
            ready_to_publish=len(included) > 0 and campaign.content is not None,
        )
        campaign.package = package
        campaign.status = CampaignStatus.ASSEMBLED
        return package

    def status(self, campaign_id: str) -> dict[str, Any]:
        campaign = self.get(campaign_id)
        verdicts = [c.verdict for c in campaign.statements]
        return {
            "campaign_id": campaign.id,
            "topic": campaign.topic,
            "status": campaign.status.value,
            "statements": {
                "total": len(campaign.statements),
                "verified": verdicts.count(Verdict.VERIFIED),
                "rejected": verdicts.count(Verdict.REJECTED),
                "needs_revision": verdicts.count(Verdict.NEEDS_REVISION),
                "unverified": verdicts.count(None),
            },
            "media": {
                "total": len(campaign.media),
                "images": sum(1 for m in campaign.media if m.media_type == "image"),
            },
            "content": {
                "has_headline": campaign.content is not None and bool(campaign.content.headline),
                "has_body": campaign.content is not None and bool(campaign.content.body),
                "has_instagram_caption": campaign.content is not None and bool(campaign.content.instagram_caption),
                "has_call_to_action": campaign.content is not None and bool(campaign.content.call_to_action),
            },
            "ready_to_assemble": (
                any(c.verdict == Verdict.VERIFIED for c in campaign.statements)
                and campaign.content is not None
                and len(campaign.media) > 0
            ),
            "ready_to_publish": (
                campaign.package is not None
                and campaign.package.ready_to_publish
            ),
        }

    def full_state(self, campaign_id: str) -> dict[str, Any]:
        campaign = self.get(campaign_id)
        return {
            "campaign_id": campaign.id,
            "topic": campaign.topic,
            "status": campaign.status.value,
            "statements": [
                {
                    "id": c.id,
                    "text": c.text,
                    "source_url": c.source_url,
                    "source_domain": c.source_domain,
                    "domain_tier": c.domain_tier,
                    "verdict": c.verdict.value if c.verdict else None,
                    "verdict_reason": c.verdict_reason,
                }
                for c in campaign.statements
            ],
            "media": [
                {
                    "id": m.id,
                    "media_type": m.media_type,
                    "ref": m.ref,
                    "description": m.description,
                }
                for m in campaign.media
            ],
            "content": (
                {
                    "id": campaign.content.id,
                    "headline": campaign.content.headline,
                    "body": campaign.content.body,
                    "instagram_caption": campaign.content.instagram_caption,
                    "call_to_action": campaign.content.call_to_action,
                }
                if campaign.content
                else None
            ),
            "package": (
                {
                    "id": campaign.package.id,
                    "included_statements": campaign.package.included_statements,
                    "excluded_statements": campaign.package.excluded_statements,
                    "verified_statements": campaign.package.verified_statements,
                    "has_content": campaign.package.has_content,
                    "media_count": campaign.package.media_count,
                    "ready_to_publish": campaign.package.ready_to_publish,
                }
                if campaign.package
                else None
            ),
        }

    def publish_draft(self, campaign_id: str) -> dict[str, str]:
        campaign = self.get(campaign_id)
        if campaign.package is None:
            raise ValueError(f"Campaign '{campaign_id}' has no assembled package. Call doc_assemble first.")
        campaign.status = CampaignStatus.DRAFT_SAVED
        return {"status": "draft_saved", "package_id": campaign.package.id}

    def publish_live(self, campaign_id: str) -> dict[str, str]:
        campaign = self.get(campaign_id)
        if campaign.package is None:
            raise ValueError(f"Campaign '{campaign_id}' has no assembled package. Call doc_assemble first.")
        if not campaign.package.ready_to_publish:
            raise ValueError("Package is not ready to publish: missing verified statements or content.")
        if campaign.package.verified_statements < 1:
            raise ValueError("At least 1 verified statement required for live publishing.")
        # NOTE: human approval (id_token/userinfo_token) validation will be added with Cedarling
        campaign.status = CampaignStatus.PUBLISHED
        return {"status": "published", "package_id": campaign.package.id}


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()
