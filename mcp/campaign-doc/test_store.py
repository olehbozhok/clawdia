"""Tests for CampaignStore."""

from store import CampaignStore, CampaignStatus, Verdict


def make_store_with_statements() -> tuple[CampaignStore, str]:
    """Helper: create store with a campaign and 2 statements."""
    store = CampaignStore()
    campaign = store.create("Test topic")
    store.add_statement(campaign.id, "Statement one", "https://noaa.gov/1", "noaa.gov", "government_scientific")
    store.add_statement(campaign.id, "Statement two", "https://fao.org/2", "fao.org", "government_scientific")
    return store, campaign.id


# ── doc_create ──

def test_create_campaign() -> None:
    store = CampaignStore()
    c = store.create("Bottom trawling impact")
    assert c.id == "camp-1"
    assert c.topic == "Bottom trawling impact"
    assert c.status == CampaignStatus.CREATED


def test_create_auto_increments() -> None:
    store = CampaignStore()
    c1 = store.create("First")
    c2 = store.create("Second")
    assert c1.id == "camp-1"
    assert c2.id == "camp-2"


# ── doc_add_statement ──

def test_add_statement() -> None:
    store = CampaignStore()
    c = store.create("Topic")
    statement = store.add_statement(c.id, "Statement", "https://noaa.gov/1", "noaa.gov", "government_scientific")
    assert statement.id == "s1"
    assert statement.verdict is None


def test_statement_ids_auto_increment() -> None:
    store, cid = make_store_with_statements()
    assert store.get(cid).statements[0].id == "s1"
    assert store.get(cid).statements[1].id == "s2"


def test_add_statement_sets_in_progress() -> None:
    store = CampaignStore()
    c = store.create("Topic")
    store.add_statement(c.id, "Statement", "https://noaa.gov/1", "noaa.gov", "tier1")
    assert store.get(c.id).status == CampaignStatus.IN_PROGRESS


# ── doc_set_verdict ──

def test_set_verdict_verified() -> None:
    store, cid = make_store_with_statements()
    statement = store.set_verdict(cid, "s1", Verdict.VERIFIED, "Matches source")
    assert statement.verdict == Verdict.VERIFIED
    assert statement.verdict_reason == "Matches source"


def test_set_verdict_needs_revision() -> None:
    store, cid = make_store_with_statements()
    statement = store.set_verdict(cid, "s2", Verdict.NEEDS_REVISION, "Missing qualifier")
    assert statement.verdict == Verdict.NEEDS_REVISION


def test_set_verdict_unknown_statement_raises() -> None:
    store, cid = make_store_with_statements()
    try:
        store.set_verdict(cid, "s99", Verdict.VERIFIED, "nope")
        assert False, "Should have raised"
    except KeyError:
        pass


# ── doc_add_media ──

def test_add_media() -> None:
    store = CampaignStore()
    c = store.create("Topic")
    media = store.add_media(c.id, "image", "/img/hero.png", "Hero image")
    assert media.id == "m1"
    assert media.media_type == "image"


# ── doc_write_content ──

def test_write_content() -> None:
    store = CampaignStore()
    c = store.create("Topic")
    content = store.write_content(c.id, "Headline", "Body", "Caption", "CTA")
    assert content.id == "t1"
    assert content.headline == "Headline"


def test_write_content_overwrites() -> None:
    store = CampaignStore()
    c = store.create("Topic")
    store.write_content(c.id, "V1", "Body1", "Cap1", "CTA1")
    store.write_content(c.id, "V2", "Body2", "Cap2", "CTA2")
    assert store.get(c.id).content is not None
    assert store.get(c.id).content.headline == "V2"
    assert store.get(c.id).content.id == "t2"


# ── doc_status ──

def test_status_empty_campaign() -> None:
    store = CampaignStore()
    c = store.create("Topic")
    s = store.status(c.id)
    assert s["statements"]["total"] == 0
    assert s["ready_to_assemble"] is False
    assert s["ready_to_publish"] is False


def test_status_with_statements() -> None:
    store, cid = make_store_with_statements()
    store.set_verdict(cid, "s1", Verdict.VERIFIED, "ok")
    s = store.status(cid)
    assert s["statements"]["total"] == 2
    assert s["statements"]["verified"] == 1
    assert s["statements"]["unverified"] == 1


def test_status_ready_to_assemble() -> None:
    store, cid = make_store_with_statements()
    store.set_verdict(cid, "s1", Verdict.VERIFIED, "ok")
    store.add_media(cid, "image", "/img.png", "img")
    store.write_content(cid, "H", "B", "C", "CTA")
    s = store.status(cid)
    assert s["ready_to_assemble"] is True


# ── doc_assemble ──

def test_assemble_includes_only_verified() -> None:
    store, cid = make_store_with_statements()
    store.set_verdict(cid, "s1", Verdict.VERIFIED, "ok")
    store.set_verdict(cid, "s2", Verdict.NEEDS_REVISION, "fix")
    store.write_content(cid, "H", "B", "C", "CTA")
    pkg = store.assemble(cid)
    assert pkg.included_statements == ["s1"]
    assert pkg.excluded_statements == ["s2"]
    assert pkg.verified_statements == 1


def test_assemble_sets_status() -> None:
    store, cid = make_store_with_statements()
    store.set_verdict(cid, "s1", Verdict.VERIFIED, "ok")
    store.write_content(cid, "H", "B", "C", "CTA")
    store.assemble(cid)
    assert store.get(cid).status == CampaignStatus.ASSEMBLED


def test_assemble_ready_to_publish_with_content() -> None:
    store, cid = make_store_with_statements()
    store.set_verdict(cid, "s1", Verdict.VERIFIED, "ok")
    store.write_content(cid, "H", "B", "C", "CTA")
    pkg = store.assemble(cid)
    assert pkg.ready_to_publish is True


def test_assemble_not_ready_without_content() -> None:
    store, cid = make_store_with_statements()
    store.set_verdict(cid, "s1", Verdict.VERIFIED, "ok")
    pkg = store.assemble(cid)
    assert pkg.ready_to_publish is False


# ── doc_publish_draft ──

def test_publish_draft() -> None:
    store, cid = make_store_with_statements()
    store.set_verdict(cid, "s1", Verdict.VERIFIED, "ok")
    store.write_content(cid, "H", "B", "C", "CTA")
    store.assemble(cid)
    result = store.publish_draft(cid)
    assert result["status"] == "draft_saved"
    assert store.get(cid).status == CampaignStatus.DRAFT_SAVED


def test_publish_draft_without_assemble_raises() -> None:
    store = CampaignStore()
    c = store.create("Topic")
    try:
        store.publish_draft(c.id)
        assert False, "Should have raised"
    except ValueError:
        pass


# ── doc_publish_live ──

def test_publish_live() -> None:
    store, cid = make_store_with_statements()
    store.set_verdict(cid, "s1", Verdict.VERIFIED, "ok")
    store.write_content(cid, "H", "B", "C", "CTA")
    store.assemble(cid)
    result = store.publish_live(cid)
    assert result["status"] == "published"
    assert store.get(cid).status == CampaignStatus.PUBLISHED


def test_publish_live_without_assemble_raises() -> None:
    store = CampaignStore()
    c = store.create("Topic")
    try:
        store.publish_live(c.id)
        assert False, "Should have raised"
    except ValueError:
        pass


def test_publish_live_without_content_raises() -> None:
    store, cid = make_store_with_statements()
    store.set_verdict(cid, "s1", Verdict.VERIFIED, "ok")
    store.assemble(cid)
    # package.ready_to_publish is False (no content)
    try:
        store.publish_live(cid)
        assert False, "Should have raised"
    except ValueError:
        pass


# ── doc_get ──

def test_full_state() -> None:
    store, cid = make_store_with_statements()
    store.set_verdict(cid, "s1", Verdict.VERIFIED, "ok")
    store.add_media(cid, "image", "/img.png", "Hero")
    store.write_content(cid, "H", "B", "C", "CTA")
    state = store.full_state(cid)
    assert len(state["statements"]) == 2
    assert state["statements"][0]["verdict"] == "verified"
    assert state["statements"][1]["verdict"] is None
    assert len(state["media"]) == 1
    assert state["content"]["headline"] == "H"


# ── unknown campaign ──

def test_get_unknown_campaign_raises() -> None:
    store = CampaignStore()
    try:
        store.get("camp-999")
        assert False, "Should have raised"
    except KeyError:
        pass
