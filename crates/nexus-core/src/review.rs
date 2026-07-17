//! 本文件实现知识卡片 ReviewState 仓储、FSRS 调度、复习统计与到期事件。

use rusqlite::{OptionalExtension, params};
use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::{
    CoreError, CoreEvent, GradeResult, MemoryKind, MemoryStore, Rating, Result, ReviewPhase,
    ReviewState, ReviewStats, ingest::current_timestamp_millis, store::enum_json,
};

const DAY_MS: i64 = 86_400_000;
const MINUTE_MS: i64 = 60_000;

/// FSRS-4.5 官方默认参数。参数、稳定度和难度始终只保存在本地数据库中。
///
/// 该版本采用 Anki/FSRS 使用的 17 个默认权重，并以 90% 为默认目标可回忆率。
/// 后续个性化拟合只需要替换这组权重，不会改变已有 `ReviewState` 的存储结构。
const FSRS_45_DEFAULT_WEIGHTS: [f64; 17] = [
    0.40255, 1.18385, 3.173, 15.69105, 7.1949, 0.5345, 1.4604, 0.0046, 1.54575, 0.1192, 1.01925,
    1.9395, 0.11, 0.29605, 2.2698, 0.2315, 2.9898,
];
const FSRS_DEFAULT_DESIRED_RETENTION: f64 = 0.9;
const FSRS_DECAY: f64 = -0.5;
const FSRS_FACTOR: f64 = 19.0 / 81.0;

impl MemoryStore {
    /// 为 `kind=card` 的 Memory 建立初始 ReviewState，并使其立即进入新卡队列。
    pub fn create_review_state(
        &self,
        memory_id: Uuid,
        card_front: impl Into<String>,
        card_back: impl Into<String>,
        deck: Option<String>,
    ) -> Result<ReviewState> {
        let memory = self
            .get(&memory_id)?
            .ok_or(CoreError::NotFound(memory_id))?;
        if memory.kind != MemoryKind::Card {
            return Err(CoreError::InvalidInput(
                "只有 kind=card 的 Memory 可以建立复习状态".into(),
            ));
        }
        let card_front = validated_card_side(card_front.into(), "卡片正面")?;
        let card_back = validated_card_side(card_back.into(), "卡片背面")?;
        let deck = deck.and_then(|value| {
            let value = value.trim().to_owned();
            (!value.is_empty()).then_some(value)
        });
        let now = current_timestamp_millis()?;
        let review = ReviewState {
            memory_id,
            card_front,
            card_back,
            stability: 0.0,
            difficulty: 5.0,
            due_at: now,
            last_reviewed_at: None,
            reps: 0,
            lapses: 0,
            state: ReviewPhase::New,
            deck,
            created_at: now,
        };
        self.connection()?.execute(
            "INSERT INTO review_states (memory_id, card_front, card_back, stability, difficulty, due_at, last_reviewed_at, reps, lapses, state, deck, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![review.memory_id.to_string(), review.card_front, review.card_back, review.stability, review.difficulty, review.due_at, review.last_reviewed_at, review.reps, review.lapses, enum_json(&review.state)?, review.deck, review.created_at],
        )?;
        Ok(review)
    }

    /// 读取指定卡片的复习状态。
    pub fn get_review_state(&self, memory_id: Uuid) -> Result<Option<ReviewState>> {
        self.connection()?
            .query_row(
                "SELECT memory_id, card_front, card_back, stability, difficulty, due_at, last_reviewed_at, reps, lapses, state, deck, created_at FROM review_states WHERE memory_id=?1",
                params![memory_id.to_string()],
                parse_review_row,
            )
            .optional()
            .map_err(Into::into)
    }

    /// 返回全部卡片状态，默认按到期时间和创建时间排序。
    pub fn list_review_states(&self, limit: usize) -> Result<Vec<ReviewState>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT memory_id, card_front, card_back, stability, difficulty, due_at, last_reviewed_at, reps, lapses, state, deck, created_at FROM review_states ORDER BY due_at, created_at, memory_id LIMIT ?1",
        )?;
        statement
            .query_map(params![bounded_limit(limit)], parse_review_row)?
            .map(|row| row.map_err(Into::into))
            .collect()
    }

    /// 返回指定时刻已经到期的复习队列。
    pub fn reviews_due(&self, now: i64, limit: usize) -> Result<Vec<ReviewState>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT memory_id, card_front, card_back, stability, difficulty, due_at, last_reviewed_at, reps, lapses, state, deck, created_at FROM review_states WHERE due_at <= ?1 ORDER BY due_at, created_at, memory_id LIMIT ?2",
        )?;
        statement
            .query_map(params![now, bounded_limit(limit)], parse_review_row)?
            .map(|row| row.map_err(Into::into))
            .collect()
    }

    /// 对卡片执行一次 FSRS 评分，原子写入新状态和评分历史。
    pub fn grade_review(
        &self,
        memory_id: Uuid,
        rating: Rating,
        reviewed_at: i64,
    ) -> Result<GradeResult> {
        let current = self
            .get_review_state(memory_id)?
            .ok_or(CoreError::NotFound(memory_id))?;
        let (updated, result) = schedule_review(&current, rating, reviewed_at);
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "UPDATE review_states SET stability=?2, difficulty=?3, due_at=?4, last_reviewed_at=?5, reps=?6, lapses=?7, state=?8, last_due_notified_at=NULL WHERE memory_id=?1",
            params![memory_id.to_string(), updated.stability, updated.difficulty, updated.due_at, updated.last_reviewed_at, updated.reps, updated.lapses, enum_json(&updated.state)?],
        )?;
        transaction.execute(
            "INSERT INTO review_logs (memory_id, rating, reviewed_at, stability, difficulty, due_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![memory_id.to_string(), enum_json(&rating)?, reviewed_at, updated.stability, updated.difficulty, updated.due_at],
        )?;
        transaction.commit()?;
        self.events.publish(CoreEvent::ReviewGraded {
            id: memory_id,
            due_at: updated.due_at,
        })?;
        Ok(result)
    }

    /// 聚合当前到期量、今日学习、成熟度与连续复习天数。
    pub fn review_stats(&self, now: i64) -> Result<ReviewStats> {
        let day_start = now - now.rem_euclid(DAY_MS);
        let connection = self.connection()?;
        let (due_today, new_today, mature, young, total_cards) = connection.query_row(
            "SELECT SUM(CASE WHEN due_at <= ?1 THEN 1 ELSE 0 END), SUM(CASE WHEN created_at >= ?2 THEN 1 ELSE 0 END), SUM(CASE WHEN stability >= 21 THEN 1 ELSE 0 END), SUM(CASE WHEN reps > 0 AND stability < 21 THEN 1 ELSE 0 END), COUNT(*) FROM review_states",
            params![now, day_start],
            |row| Ok((row.get::<_, Option<usize>>(0)?.unwrap_or(0), row.get::<_, Option<usize>>(1)?.unwrap_or(0), row.get::<_, Option<usize>>(2)?.unwrap_or(0), row.get::<_, Option<usize>>(3)?.unwrap_or(0), row.get::<_, usize>(4)?)),
        )?;
        let reviewed_today = connection.query_row(
            "SELECT COUNT(*) FROM review_logs WHERE reviewed_at >= ?1",
            params![day_start],
            |row| row.get(0),
        )?;
        let mut statement = connection.prepare(
            "SELECT DISTINCT CAST(reviewed_at / ?1 AS INTEGER) FROM review_logs ORDER BY 1 DESC",
        )?;
        let review_days = statement
            .query_map(params![DAY_MS], |row| row.get::<_, i64>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(ReviewStats {
            due_today,
            new_today,
            reviewed_today,
            streak: calculate_streak(&review_days, now.div_euclid(DAY_MS)),
            mature,
            young,
            total_cards,
        })
    }

    /// 为尚未通知的到期卡片发布 `ReviewDue`，并记录本轮到期已通知。
    pub fn notify_due_reviews(&self, now: i64, limit: usize) -> Result<usize> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        let due = {
            let mut statement = transaction.prepare(
                "SELECT memory_id, due_at FROM review_states WHERE due_at <= ?1 AND (last_due_notified_at IS NULL OR last_due_notified_at < due_at) ORDER BY due_at LIMIT ?2",
            )?;
            statement
                .query_map(params![now, bounded_limit(limit)], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        for (id, _) in &due {
            transaction.execute(
                "UPDATE review_states SET last_due_notified_at=?2 WHERE memory_id=?1",
                params![id, now],
            )?;
        }
        transaction.commit()?;
        for (id, due_at) in &due {
            self.events.publish(CoreEvent::ReviewDue {
                id: Uuid::parse_str(id).map_err(|error| {
                    CoreError::InvalidInput(format!("复习状态 UUID 无效: {error}"))
                })?,
                due_at: *due_at,
            })?;
        }
        Ok(due.len())
    }
}

/// 根据当前状态、评分和复习时间计算下一次 FSRS 调度结果。
#[must_use]
pub fn schedule_review(
    current: &ReviewState,
    rating: Rating,
    reviewed_at: i64,
) -> (ReviewState, GradeResult) {
    let mut updated = current.clone();
    let was_new = current.reps == 0;

    let interval_ms = if was_new {
        updated.stability = initial_stability(rating);
        updated.difficulty = initial_difficulty(rating);
        match rating {
            Rating::Again => {
                updated.state = ReviewPhase::Learning;
                10 * MINUTE_MS
            }
            Rating::Hard => {
                updated.state = ReviewPhase::Learning;
                DAY_MS
            }
            Rating::Good | Rating::Easy => {
                updated.state = ReviewPhase::Review;
                interval_to_millis(updated.stability)
            }
        }
    } else {
        let elapsed_days = current.last_reviewed_at.map_or(0.0, |last| {
            ((reviewed_at - last).max(0) as f64 / DAY_MS as f64).max(0.0)
        });
        let retrievability = retrievability(current.stability, elapsed_days);
        updated.difficulty = next_difficulty(current.difficulty, rating);
        match rating {
            Rating::Again => {
                updated.stability =
                    next_forget_stability(current.stability, updated.difficulty, retrievability);
                updated.state = ReviewPhase::Relearning;
                updated.lapses = updated.lapses.saturating_add(1);
                10 * MINUTE_MS
            }
            Rating::Hard => {
                updated.stability = next_recall_stability(
                    current.stability,
                    updated.difficulty,
                    retrievability,
                    rating,
                );
                updated.state = ReviewPhase::Review;
                interval_to_millis(updated.stability)
            }
            Rating::Good => {
                updated.stability = next_recall_stability(
                    current.stability,
                    updated.difficulty,
                    retrievability,
                    rating,
                );
                updated.state = ReviewPhase::Review;
                interval_to_millis(updated.stability)
            }
            Rating::Easy => {
                updated.stability = next_recall_stability(
                    current.stability,
                    updated.difficulty,
                    retrievability,
                    rating,
                );
                updated.state = ReviewPhase::Review;
                interval_to_millis(updated.stability)
            }
        }
    };
    updated.reps = updated.reps.saturating_add(1);
    updated.last_reviewed_at = Some(reviewed_at);
    updated.due_at = reviewed_at.saturating_add(interval_ms);
    let result = GradeResult {
        next_due_at: updated.due_at,
        new_stability: updated.stability,
        new_difficulty: updated.difficulty,
        new_state: updated.state,
    };
    (updated, result)
}

/// 计算新卡按 Again/Hard/Good/Easy 首次评分得到的官方 FSRS 初始稳定度。
fn initial_stability(rating: Rating) -> f64 {
    FSRS_45_DEFAULT_WEIGHTS[usize::from(rating.value() - 1)]
}

/// 计算新卡初始难度；评分越轻松，初始难度越低。
fn initial_difficulty(rating: Rating) -> f64 {
    let weights = FSRS_45_DEFAULT_WEIGHTS;
    (weights[4] - (f64::from(rating.value()) - 3.0) * weights[5]).clamp(1.0, 10.0)
}

/// 使用 FSRS 的线性难度漂移与均值回归更新下一次难度。
fn next_difficulty(difficulty: f64, rating: Rating) -> f64 {
    let weights = FSRS_45_DEFAULT_WEIGHTS;
    let delta = difficulty - weights[6] * (f64::from(rating.value()) - 3.0);
    (weights[7] * initial_difficulty(Rating::Again) + (1.0 - weights[7]) * delta).clamp(1.0, 10.0)
}

/// 根据稳定度和经过天数计算 FSRS 可回忆率，避免负时钟造成异常值。
fn retrievability(stability: f64, elapsed_days: f64) -> f64 {
    if stability <= 0.0 {
        return 0.0;
    }
    (1.0 + FSRS_FACTOR * elapsed_days.max(0.0) / stability).powf(FSRS_DECAY)
}

/// 计算成功回忆后的稳定度增长，Hard/Easy 使用 FSRS 对应的惩罚与奖励权重。
fn next_recall_stability(
    stability: f64,
    difficulty: f64,
    retrievability: f64,
    rating: Rating,
) -> f64 {
    let weights = FSRS_45_DEFAULT_WEIGHTS;
    let hard_penalty = if rating == Rating::Hard {
        weights[15]
    } else {
        1.0
    };
    let easy_bonus = if rating == Rating::Easy {
        weights[16]
    } else {
        1.0
    };
    let growth = 1.0
        + weights[8].exp()
            * (11.0 - difficulty)
            * stability.max(0.1).powf(-weights[9])
            * ((1.0 - retrievability).max(0.0) * weights[10]).exp_m1()
            * hard_penalty
            * easy_bonus;
    (stability * growth).clamp(0.1, 36_500.0)
}

/// 计算遗忘后的稳定度，遗忘次数越多或回忆率越低时会进入更保守的复习节奏。
fn next_forget_stability(stability: f64, difficulty: f64, retrievability: f64) -> f64 {
    let weights = FSRS_45_DEFAULT_WEIGHTS;
    (weights[11]
        * difficulty.max(1.0).powf(-weights[12])
        * ((stability.max(0.1) + 1.0).powf(weights[13]) - 1.0)
        * ((1.0 - retrievability).max(0.0) * weights[14]).exp())
    .clamp(0.1, 36_500.0)
}

/// 将 FSRS 稳定度换算为达到目标可回忆率的下次间隔，并转换为安全毫秒值。
fn interval_to_millis(stability: f64) -> i64 {
    let days =
        stability / FSRS_FACTOR * (FSRS_DEFAULT_DESIRED_RETENTION.powf(1.0 / FSRS_DECAY) - 1.0);
    (days.clamp(1.0 / 144.0, 36_500.0) * DAY_MS as f64).round() as i64
}

/// 计算以今天或昨天为结尾的连续复习天数。
fn calculate_streak(days_desc: &[i64], today: i64) -> usize {
    let Some(&latest) = days_desc.first() else {
        return 0;
    };
    if latest < today - 1 {
        return 0;
    }
    days_desc
        .iter()
        .zip((0_i64..).map(|offset| latest - offset))
        .take_while(|(actual, expected)| **actual == *expected)
        .count()
}

/// 校验卡片正反面内容并去除首尾空白。
fn validated_card_side(value: String, label: &str) -> Result<String> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(CoreError::InvalidInput(format!("{label}不能为空")));
    }
    Ok(value)
}

/// 把用户传入的列表上限约束在可控范围内。
fn bounded_limit(limit: usize) -> usize {
    limit.clamp(1, 500)
}

/// 从 SQLite 行恢复 ReviewState。
fn parse_review_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReviewState> {
    let id = row.get::<_, String>(0)?;
    let state = row.get::<_, String>(9)?;
    Ok(ReviewState {
        memory_id: Uuid::parse_str(&id).map_err(sql_conversion_error)?,
        card_front: row.get(1)?,
        card_back: row.get(2)?,
        stability: row.get(3)?,
        difficulty: row.get(4)?,
        due_at: row.get(5)?,
        last_reviewed_at: row.get(6)?,
        reps: row.get(7)?,
        lapses: row.get(8)?,
        state: parse_stored_enum(&state).map_err(sql_conversion_error)?,
        deck: row.get(10)?,
        created_at: row.get(11)?,
    })
}

/// 从数据库 snake_case 字符串恢复可反序列化枚举。
fn parse_stored_enum<T: DeserializeOwned>(value: &str) -> serde_json::Result<T> {
    serde_json::from_str(&format!("\"{value}\""))
}

/// 把模型解析错误转换为 SQLite 列转换错误。
fn sql_conversion_error(error: impl std::error::Error + Send + Sync + 'static) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}
