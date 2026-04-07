"""Campaign document store with optional file persistence."""

from __future__ import annotations

import json
from dataclasses import asdict, dataclass, field
from datetime import datetime, timezone
from enum import Enum
from pathlib import Path
from typing import Any
from urllib.parse import urlparse


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
    def __init__(self, storage_dir: Path | None = None) -> None:
        self._campaigns: dict[str, Campaign] = {}
        self._counter: int = 0
        self._storage_dir = storage_dir
        if storage_dir is not None:
            storage_dir.mkdir(parents=True, exist_ok=True)
            self._load_all()

    def _next_id(self) -> str:
        self._counter += 1
        candidate = f"camp-{self._counter}"
        while candidate in self._campaigns:
            self._counter += 1
            candidate = f"camp-{self._counter}"
        return candidate

    def create(self, topic: str) -> Campaign:
        for existing in self._campaigns.values():
            if existing.topic.lower() == topic.lower():
                raise ValueError(
                    f"Campaign with topic '{topic}' already exists (id={existing.id})"
                )
        campaign = Campaign(
            id=self._next_id(),
            topic=topic,
            status=CampaignStatus.CREATED,
        )
        self._campaigns[campaign.id] = campaign
        self._save(campaign)
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
    ) -> Statement:
        campaign = self.get(campaign_id)
        statement = Statement(
            id=campaign.next_statement_id(),
            text=text,
            source_url=source_url,
            source_domain=_extract_domain(source_url),
        )
        campaign.statements.append(statement)
        campaign.status = CampaignStatus.IN_PROGRESS
        self._save(campaign)
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
                self._save(campaign)
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
        self._save(campaign)
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
        self._save(campaign)
        return content

    def assemble(self, campaign_id: str) -> CampaignPackage:
        campaign = self.get(campaign_id)
        included = [s.id for s in campaign.statements if s.verdict == Verdict.VERIFIED]
        excluded = [s.id for s in campaign.statements if s.verdict != Verdict.VERIFIED]

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
        self._save(campaign)
        return package

    def list_campaigns(self, status: CampaignStatus | None = None) -> list[dict[str, Any]]:
        campaigns = self._campaigns.values()
        if status is not None:
            campaigns = [c for c in campaigns if c.status == status]
        return [
            {
                "campaign_id": c.id,
                "topic": c.topic,
                "status": c.status.value,
                "statements": len(c.statements),
                "created_at": c.created_at,
            }
            for c in campaigns
        ]

    def status(self, campaign_id: str) -> dict[str, Any]:
        campaign = self.get(campaign_id)
        verdicts = [s.verdict for s in campaign.statements]
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
                any(s.verdict == Verdict.VERIFIED for s in campaign.statements)
                and campaign.content is not None
                and len(campaign.media) > 0
            ),
            "ready_to_publish": (
                campaign.package is not None
                and campaign.package.ready_to_publish
            ),
        }

    def list_statements(
        self,
        campaign_id: str,
        verdict_filter: str | None = None,
    ) -> list[dict[str, Any]]:
        campaign = self.get(campaign_id)
        statements = campaign.statements
        if verdict_filter == "unverified":
            statements = [s for s in statements if s.verdict is None]
        elif verdict_filter is not None:
            v = Verdict(verdict_filter)
            statements = [s for s in statements if s.verdict == v]
        return [
            {
                "statement_id": s.id,
                "text": s.text,
                "source_url": s.source_url,
                "source_domain": s.source_domain,
                "verdict": s.verdict.value if s.verdict else None,
                "verdict_reason": s.verdict_reason,
            }
            for s in statements
        ]

    def verified_state(self, campaign_id: str) -> dict[str, Any]:
        """Return campaign state with only verified statements."""
        campaign = self.get(campaign_id)
        verified = [s for s in campaign.statements if s.verdict == Verdict.VERIFIED]
        return {
            "campaign_id": campaign.id,
            "topic": campaign.topic,
            "status": campaign.status.value,
            "verified_statements": [
                {
                    "id": s.id,
                    "text": s.text,
                    "source_url": s.source_url,
                    "source_domain": s.source_domain,
                }
                for s in verified
            ],
            "verified_count": len(verified),
            "total_statements": len(campaign.statements),
        }

    def full_state(self, campaign_id: str) -> dict[str, Any]:
        campaign = self.get(campaign_id)
        return {
            "campaign_id": campaign.id,
            "topic": campaign.topic,
            "status": campaign.status.value,
            "statements": [
                {
                    "id": s.id,
                    "text": s.text,
                    "source_url": s.source_url,
                    "source_domain": s.source_domain,
                    "verdict": s.verdict.value if s.verdict else None,
                    "verdict_reason": s.verdict_reason,
                }
                for s in campaign.statements
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
        self._save(campaign)
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
        self._save(campaign)
        return {"status": "published", "package_id": campaign.package.id}


    # -- Persistence helpers --------------------------------------------------

    def _campaign_path(self, campaign_id: str) -> Path:
        assert self._storage_dir is not None
        return self._storage_dir / f"{campaign_id}.json"

    def _save(self, campaign: Campaign) -> None:
        if self._storage_dir is None:
            return
        data = asdict(campaign)
        data.pop("_statement_counter", None)
        data.pop("_media_counter", None)
        data.pop("_content_counter", None)
        data.pop("_package_counter", None)
        data["_counters"] = {
            "statement": campaign._statement_counter,
            "media": campaign._media_counter,
            "content": campaign._content_counter,
            "package": campaign._package_counter,
        }
        self._campaign_path(campaign.id).write_text(
            json.dumps(data, indent=2, ensure_ascii=False),
            encoding="utf-8",
        )

    def _load_all(self) -> None:
        assert self._storage_dir is not None
        for path in sorted(self._storage_dir.glob("camp-*.json")):
            data = json.loads(path.read_text(encoding="utf-8"))
            campaign = _campaign_from_dict(data)
            self._campaigns[campaign.id] = campaign
            num = int(campaign.id.removeprefix("camp-"))
            if num > self._counter:
                self._counter = num


def _campaign_from_dict(data: dict[str, Any]) -> Campaign:
    counters = data.pop("_counters", {})
    statements = [
        Statement(
            id=s["id"],
            text=s["text"],
            source_url=s["source_url"],
            source_domain=s["source_domain"],
            verdict=Verdict(s["verdict"]) if s.get("verdict") else None,
            verdict_reason=s.get("verdict_reason"),
            created_at=s.get("created_at", _now()),
        )
        for s in data.get("statements", [])
    ]
    media = [
        MediaRef(
            id=m["id"],
            media_type=m["media_type"],
            ref=m["ref"],
            description=m["description"],
            created_at=m.get("created_at", _now()),
        )
        for m in data.get("media", [])
    ]
    content_data = data.get("content")
    content = (
        CampaignContent(
            id=content_data["id"],
            headline=content_data["headline"],
            body=content_data["body"],
            instagram_caption=content_data["instagram_caption"],
            call_to_action=content_data["call_to_action"],
            created_at=content_data.get("created_at", _now()),
        )
        if content_data
        else None
    )
    package_data = data.get("package")
    package = (
        CampaignPackage(
            id=package_data["id"],
            included_statements=package_data["included_statements"],
            excluded_statements=package_data["excluded_statements"],
            verified_statements=package_data["verified_statements"],
            has_content=package_data["has_content"],
            media_count=package_data["media_count"],
            ready_to_publish=package_data["ready_to_publish"],
            assembled_at=package_data.get("assembled_at", _now()),
        )
        if package_data
        else None
    )
    campaign = Campaign(
        id=data["id"],
        topic=data["topic"],
        status=CampaignStatus(data["status"]),
        statements=statements,
        media=media,
        content=content,
        package=package,
        created_at=data.get("created_at", _now()),
    )
    campaign._statement_counter = counters.get("statement", len(statements))
    campaign._media_counter = counters.get("media", len(media))
    campaign._content_counter = counters.get("content", 1 if content else 0)
    campaign._package_counter = counters.get("package", 1 if package else 0)
    return campaign


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _extract_domain(url: str) -> str:
    parsed = urlparse(url)
    if not parsed.hostname:
        raise ValueError(f"Cannot extract domain from URL: {url}")
    return parsed.hostname
