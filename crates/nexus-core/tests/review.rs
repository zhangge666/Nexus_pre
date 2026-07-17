//! 本文件验证 M4 知识卡片创建、FSRS 调度、到期通知、统计与级联清理闭环。

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use nexus_core::{
    ContentFormat, CoreEvent, CreateCardInput, HashEmbedder, IngestInput, Ingestor, LinkCreator,
    LinkRelation, MemoryKind, MemorySource, MemoryStore, Rating, ReviewPhase, ReviewState,
    review::schedule_review,
};

const DAY_MS: i64 = 86_400_000;

/// 返回测试评分使用的当前 Unix 毫秒时间。
fn now_millis() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("测试系统时间应晚于 Unix epoch")
            .as_millis(),
    )
    .expect("测试时间戳应处于 i64 范围")
}

/// 创建测试用的普通来源记忆。
fn create_source(store: &MemoryStore, embedder: &HashEmbedder) -> uuid::Uuid {
    Ingestor::new(store, embedder)
        .ingest(IngestInput {
            source: MemorySource::Muse,
            kind: MemoryKind::Note,
            title: Some("FSRS 来源".into()),
            content: "FSRS 会根据回忆表现调整下一次复习间隔。".into(),
            content_format: ContentFormat::Plain,
            tags: vec!["learning".into()],
            captured_at: None,
            device_id: "review-test".into(),
            meta: serde_json::json!({}),
        })
        .expect("来源记忆应创建成功")
        .id
}

/// 创建一张带默认测试数据的知识卡片。
fn create_card(store: &MemoryStore, embedder: &HashEmbedder) -> uuid::Uuid {
    store
        .create_card(
            CreateCardInput {
                card_front: "FSRS 的用途是什么？".into(),
                card_back: "根据回忆表现安排间隔复习。".into(),
                source_memory_id: None,
                deck: Some("Nexus".into()),
                tags: vec!["review".into()],
                created_by: LinkCreator::User,
                provider: None,
            },
            embedder,
        )
        .expect("测试卡片应创建成功")
        .id
}

/// 验证卡片 Memory、派生关系和初始 ReviewState 由同一创建流程建立。
#[test]
fn creates_card_with_review_state_and_source_link() {
    let store = MemoryStore::open_in_memory().expect("内存库应创建成功");
    let embedder = HashEmbedder::default();
    let source_id = create_source(&store, &embedder);
    let card = store
        .create_card(
            CreateCardInput {
                card_front: "什么是稳定度？".into(),
                card_back: "在目标可提取性下预计保持记忆的时间。".into(),
                source_memory_id: Some(source_id),
                deck: Some("  学习科学  ".into()),
                tags: vec!["fsrs".into()],
                created_by: LinkCreator::Ai,
                provider: Some("local".into()),
            },
            &embedder,
        )
        .expect("派生卡片应创建成功");

    assert_eq!(card.kind, MemoryKind::Card);
    assert_eq!(card.source, MemorySource::Orbit);
    let review = store
        .get_review_state(card.id)
        .expect("复习状态读取应成功")
        .expect("卡片应有复习状态");
    assert_eq!(review.state, ReviewPhase::New);
    assert_eq!(review.reps, 0);
    assert_eq!(review.deck.as_deref(), Some("学习科学"));
    assert!(review.due_at >= review.created_at);

    let links = store.list_links(card.id).expect("关联读取应成功");
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].from_id, card.id);
    assert_eq!(links[0].to_id, source_id);
    assert_eq!(links[0].relation, LinkRelation::DerivedFrom);
    assert_eq!(links[0].created_by, LinkCreator::Ai);
}

/// 验证普通 Memory 不能被误加入卡片复习系统。
#[test]
fn rejects_review_state_for_non_card_memory() {
    let store = MemoryStore::open_in_memory().expect("内存库应创建成功");
    let embedder = HashEmbedder::default();
    let source_id = create_source(&store, &embedder);

    assert!(
        store
            .create_review_state(source_id, "正面", "背面", None)
            .is_err()
    );
}

/// 验证新卡四档评分形成严格递增的间隔，并正确进入学习或复习阶段。
#[test]
fn schedules_new_card_ratings_in_expected_order() {
    let store = MemoryStore::open_in_memory().expect("内存库应创建成功");
    let embedder = HashEmbedder::default();
    let card_id = create_card(&store, &embedder);
    let review = store
        .get_review_state(card_id)
        .unwrap()
        .expect("卡片应有复习状态");
    let reviewed_at = now_millis();

    let (again, _) = schedule_review(&review, Rating::Again, reviewed_at);
    let (hard, _) = schedule_review(&review, Rating::Hard, reviewed_at);
    let (good, _) = schedule_review(&review, Rating::Good, reviewed_at);
    let (easy, _) = schedule_review(&review, Rating::Easy, reviewed_at);

    assert!(again.due_at < hard.due_at);
    assert!(hard.due_at < good.due_at);
    assert!(good.due_at < easy.due_at);
    assert_eq!(again.state, ReviewPhase::Learning);
    assert_eq!(hard.state, ReviewPhase::Learning);
    assert_eq!(good.state, ReviewPhase::Review);
    assert_eq!(easy.state, ReviewPhase::Review);
}

/// 验证官方 FSRS-4.5 默认权重的关键回归向量，防止后续改动退回近似乘数调度。
#[test]
fn uses_official_fsrs_default_weight_regression_vectors() {
    let reviewed_at = 3 * DAY_MS;
    let review = ReviewState {
        memory_id: uuid::Uuid::nil(),
        card_front: "FSRS 回归向量".into(),
        card_back: "验证默认权重".into(),
        stability: 3.173,
        difficulty: 7.1949,
        due_at: reviewed_at,
        last_reviewed_at: Some(0),
        reps: 1,
        lapses: 0,
        state: ReviewPhase::Review,
        deck: None,
        created_at: 0,
    };

    let (again, _) = schedule_review(&review, Rating::Again, reviewed_at);
    let (hard, _) = schedule_review(&review, Rating::Hard, reviewed_at);
    let (good, _) = schedule_review(&review, Rating::Good, reviewed_at);
    let (easy, _) = schedule_review(&review, Rating::Easy, reviewed_at);

    assert!(
        (again.stability - 0.984_000_063).abs() < 0.000_001,
        "实际 Again 稳定度: {}",
        again.stability
    );
    assert!(
        (hard.stability - 3.891_823_988).abs() < 0.000_001,
        "实际 Hard 稳定度: {}",
        hard.stability
    );
    assert!((good.stability - 8.201_696_105).abs() < 0.000_001);
    assert!(
        (easy.stability - 23.959_049_297).abs() < 0.000_001,
        "实际 Easy 稳定度: {}",
        easy.stability
    );
    assert!(again.difficulty > review.difficulty);
    assert!(easy.difficulty < review.difficulty);
}

/// 验证评分原子更新状态、写入历史并在提交后发布事件。
#[test]
fn grades_review_and_publishes_event() {
    let store = MemoryStore::open_in_memory().expect("内存库应创建成功");
    let embedder = HashEmbedder::default();
    let card_id = create_card(&store, &embedder);
    let subscription = store.subscribe().expect("事件订阅应成功");
    let reviewed_at = now_millis();

    let result = store
        .grade_review(card_id, Rating::Good, reviewed_at)
        .expect("评分应成功");
    let updated = store
        .get_review_state(card_id)
        .unwrap()
        .expect("评分后状态应存在");
    assert_eq!(updated.reps, 1);
    assert_eq!(updated.state, ReviewPhase::Review);
    assert_eq!(updated.due_at, result.next_due_at);
    assert_eq!(
        subscription.recv_timeout(Duration::from_secs(1)),
        Some(CoreEvent::ReviewGraded {
            id: card_id,
            due_at: result.next_due_at,
        })
    );

    store
        .grade_review(card_id, Rating::Again, result.next_due_at)
        .expect("遗忘评分应成功");
    let relearning = store
        .get_review_state(card_id)
        .unwrap()
        .expect("重学状态应存在");
    assert_eq!(relearning.lapses, 1);
    assert_eq!(relearning.state, ReviewPhase::Relearning);
}

/// 验证同一到期周期只发布一次 ReviewDue，评分后可进入新通知周期。
#[test]
fn deduplicates_due_notifications_per_schedule() {
    let store = MemoryStore::open_in_memory().expect("内存库应创建成功");
    let embedder = HashEmbedder::default();
    let card_id = create_card(&store, &embedder);
    let subscription = store.subscribe().expect("事件订阅应成功");
    let now = now_millis() + 1_000;

    assert_eq!(store.notify_due_reviews(now, 20).unwrap(), 1);
    let event = subscription
        .recv_timeout(Duration::from_secs(1))
        .expect("应收到到期事件");
    assert!(matches!(event, CoreEvent::ReviewDue { id, .. } if id == card_id));
    assert_eq!(store.notify_due_reviews(now + 1_000, 20).unwrap(), 0);
    assert!(
        subscription
            .recv_timeout(Duration::from_millis(20))
            .is_none()
    );

    let result = store
        .grade_review(card_id, Rating::Again, now)
        .expect("评分应开启下一到期周期");
    assert_eq!(store.notify_due_reviews(result.next_due_at, 20).unwrap(), 1);
}

/// 验证统计覆盖新卡、当天评分、连续天数、年轻/成熟卡和总量。
#[test]
fn aggregates_review_statistics() {
    let store = MemoryStore::open_in_memory().expect("内存库应创建成功");
    let embedder = HashEmbedder::default();
    let card_id = create_card(&store, &embedder);
    let reviewed_at = now_millis();

    let first = store
        .grade_review(card_id, Rating::Easy, reviewed_at)
        .expect("首次评分应成功");
    let young = store.review_stats(reviewed_at).expect("年轻卡统计应成功");
    assert_eq!(young.new_today, 1);
    assert_eq!(young.reviewed_today, 1);
    assert_eq!(young.streak, 1);
    assert_eq!(young.young, 1);
    assert_eq!(young.mature, 0);
    assert_eq!(young.total_cards, 1);

    let second = store
        .grade_review(card_id, Rating::Easy, first.next_due_at)
        .expect("第二次评分应成功");
    let third = store
        .grade_review(card_id, Rating::Easy, second.next_due_at)
        .expect("第三次评分应成功");
    let mature = store
        .review_stats(third.next_due_at - DAY_MS)
        .expect("成熟卡统计应成功");
    assert_eq!(mature.mature, 1);
    assert_eq!(mature.young, 0);
    assert_eq!(mature.total_cards, 1);
}

/// 验证删除卡片会级联删除复习状态和评分日志。
#[test]
fn cascades_review_state_and_logs_when_card_is_deleted() {
    let store = MemoryStore::open_in_memory().expect("内存库应创建成功");
    let embedder = HashEmbedder::default();
    let card_id = create_card(&store, &embedder);
    let reviewed_at = now_millis();
    store
        .grade_review(card_id, Rating::Good, reviewed_at)
        .expect("删除前评分应成功");

    store.delete(&card_id).expect("卡片应删除成功");
    assert!(store.get_review_state(card_id).unwrap().is_none());
    let stats = store.review_stats(reviewed_at).expect("删除后统计应成功");
    assert_eq!(stats.total_cards, 0);
    assert_eq!(stats.reviewed_today, 0);
}
