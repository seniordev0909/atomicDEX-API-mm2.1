use crate::mm2::lp_swap::{stats_maker_swap_dir, stats_taker_swap_dir, MakerSavedSwap, SavedSwap, TakerSavedSwap};
use common::{log::{debug, error, warn},
             mm_ctx::MmArc,
             read_dir,
             rusqlite::{Connection, OptionalExtension},
             slurp};
use serde_json::{self as json};
use std::collections::HashSet;
use uuid::Uuid;

const CREATE_STATS_SWAPS_TABLE: &str = "CREATE TABLE IF NOT EXISTS stats_swaps (
    id INTEGER NOT NULL PRIMARY KEY,
    maker_coin VARCHAR(255) NOT NULL,
    taker_coin VARCHAR(255) NOT NULL,
    uuid VARCHAR(255) NOT NULL UNIQUE,
    started_at INTEGER NOT NULL,
    finished_at INTEGER NOT NULL,
    maker_amount DECIMAL NOT NULL,
    taker_amount DECIMAL NOT NULL,
    is_success INTEGER NOT NULL
);";

const INSERT_STATS_SWAP_ON_INIT: &str = "INSERT INTO stats_swaps (
    maker_coin,
    taker_coin,
    uuid,
    started_at,
    finished_at,
    maker_amount,
    taker_amount,
    is_success
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)";

const INSERT_STATS_SWAP: &str = "INSERT INTO stats_swaps (
    maker_coin,
    maker_coin_ticker,
    maker_coin_platform,
    taker_coin,
    taker_coin_ticker,
    taker_coin_platform,
    uuid,
    started_at,
    finished_at,
    maker_amount,
    taker_amount,
    is_success
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)";

const ADD_SPLIT_TICKERS: &[&str] = &[
    "ALTER TABLE stats_swaps ADD COLUMN maker_coin_ticker VARCHAR(255) NOT NULL DEFAULT '';",
    "ALTER TABLE stats_swaps ADD COLUMN maker_coin_platform VARCHAR(255) NOT NULL DEFAULT '';",
    "ALTER TABLE stats_swaps ADD COLUMN taker_coin_ticker VARCHAR(255) NOT NULL DEFAULT '';",
    "ALTER TABLE stats_swaps ADD COLUMN taker_coin_platform VARCHAR(255) NOT NULL DEFAULT '';",
    "UPDATE stats_swaps SET maker_coin_ticker = CASE instr(maker_coin, '-') \
        WHEN 0 THEN maker_coin \
        ELSE substr(maker_coin, 0, instr(maker_coin, '-')) \
        END;",
    "UPDATE stats_swaps SET maker_coin_platform = CASE instr(maker_coin, '-') \
        WHEN 0 THEN '' \
        ELSE substr(maker_coin, instr(maker_coin, '-') + 1) \
        END;",
    "UPDATE stats_swaps SET taker_coin_ticker = CASE instr(taker_coin, '-') \
        WHEN 0 THEN taker_coin \
        ELSE substr(taker_coin, 0, instr(taker_coin, '-')) \
        END;",
    "UPDATE stats_swaps SET taker_coin_platform = CASE instr(taker_coin, '-') \
        WHEN 0 THEN '' \
        ELSE substr(taker_coin, instr(taker_coin, '-') + 1) \
        END;",
];

pub const ADD_STARTED_AT_INDEX: &str = "CREATE INDEX timestamp_index ON stats_swaps (started_at);";

const SELECT_ID_BY_UUID: &str = "SELECT id FROM stats_swaps WHERE uuid = ?1";

/// Returns SQL statements to initially fill stats_swaps table using existing DB with JSON files
pub fn create_and_fill_stats_swaps_from_json_statements(ctx: &MmArc) -> Vec<(&'static str, Vec<String>)> {
    let maker_swap_files =
        read_dir(&stats_maker_swap_dir(ctx)).expect("Reading swaps dir should not fail at this point");
    let mut result = vec![(CREATE_STATS_SWAPS_TABLE, vec![])];
    let mut inserted_maker_uuids = HashSet::with_capacity(maker_swap_files.len());
    for (_, file) in maker_swap_files {
        let content = slurp(&file).expect("slurp should not fail at this point");
        match json::from_slice(&content) {
            Ok(swap) => {
                if let Some(sql_with_params) = insert_stats_maker_swap_sql_init(&swap) {
                    inserted_maker_uuids.insert(swap.uuid);
                    result.push(sql_with_params);
                }
            },
            Err(e) => error!(
                "Error {} on file {} content {:?} deserialization to MakerSavedSwap",
                e,
                file.display(),
                content
            ),
        }
    }

    let taker_swap_files =
        read_dir(&stats_taker_swap_dir(ctx)).expect("Reading swaps dir should not fail at this point");
    for (_, file) in taker_swap_files {
        let os_file_name = match file.file_stem() {
            Some(name) => name,
            None => {
                warn!("File {} does not have file_stem", file.display());
                continue;
            },
        };
        let file_name = match os_file_name.to_str() {
            Some(name) => name,
            None => {
                warn!("{:?} is not a valid unicode", os_file_name);
                continue;
            },
        };
        let uuid: Uuid = match file_name.parse() {
            Ok(u) => u,
            Err(e) => {
                warn!("Error {} while parsing uuid from {}", e, file_name);
                continue;
            },
        };

        if inserted_maker_uuids.contains(&uuid) {
            continue;
        }

        let content = slurp(&file).expect("slurp should not fail at this point");
        match json::from_slice(&content) {
            Ok(swap) => {
                if let Some(sql_with_params) = insert_stats_taker_swap_sql_init(&swap) {
                    result.push(sql_with_params);
                }
            },
            Err(e) => error!(
                "Error {} on file {} content {:?} deserialization to TakerSavedSwap",
                e,
                file.display(),
                content
            ),
        }
    }
    result
}

fn split_coin(coin: &str) -> (String, String) {
    let mut split = coin.split('-');
    let ticker = split.next().expect("split returns empty string at least").into();
    let platform = split.next().map_or("".into(), |platform| platform.into());
    (ticker, platform)
}

fn insert_stats_maker_swap_sql(swap: &MakerSavedSwap) -> Option<(&'static str, Vec<String>)> {
    let swap_data = match swap.swap_data() {
        Ok(d) => d,
        Err(e) => {
            error!("Error {} on getting swap {} data", e, swap.uuid);
            return None;
        },
    };
    let finished_at = match swap.finished_at() {
        Ok(t) => t.to_string(),
        Err(e) => {
            error!("Error {} on getting swap {} finished_at", e, swap.uuid);
            return None;
        },
    };
    let is_success = swap
        .is_success()
        .expect("is_success can return error only when swap is not finished");

    let (maker_coin_ticker, maker_coin_platform) = split_coin(&swap_data.maker_coin);
    let (taker_coin_ticker, taker_coin_platform) = split_coin(&swap_data.taker_coin);

    let params = vec![
        swap_data.maker_coin.clone(),
        maker_coin_ticker,
        maker_coin_platform,
        swap_data.taker_coin.clone(),
        taker_coin_ticker,
        taker_coin_platform,
        swap.uuid.to_string(),
        swap_data.started_at.to_string(),
        finished_at,
        swap_data.maker_amount.to_string(),
        swap_data.taker_amount.to_string(),
        (is_success as u32).to_string(),
    ];
    Some((INSERT_STATS_SWAP, params))
}

fn insert_stats_maker_swap_sql_init(swap: &MakerSavedSwap) -> Option<(&'static str, Vec<String>)> {
    let swap_data = match swap.swap_data() {
        Ok(d) => d,
        Err(e) => {
            error!("Error {} on getting swap {} data", e, swap.uuid);
            return None;
        },
    };
    let finished_at = match swap.finished_at() {
        Ok(t) => t.to_string(),
        Err(e) => {
            error!("Error {} on getting swap {} finished_at", e, swap.uuid);
            return None;
        },
    };
    let is_success = swap
        .is_success()
        .expect("is_success can return error only when swap is not finished");

    let params = vec![
        swap_data.maker_coin.clone(),
        swap_data.taker_coin.clone(),
        swap.uuid.to_string(),
        swap_data.started_at.to_string(),
        finished_at,
        swap_data.maker_amount.to_string(),
        swap_data.taker_amount.to_string(),
        (is_success as u32).to_string(),
    ];
    Some((INSERT_STATS_SWAP_ON_INIT, params))
}

fn insert_stats_taker_swap_sql(swap: &TakerSavedSwap) -> Option<(&'static str, Vec<String>)> {
    let swap_data = match swap.swap_data() {
        Ok(d) => d,
        Err(e) => {
            error!("Error {} on getting swap {} data", e, swap.uuid);
            return None;
        },
    };
    let finished_at = match swap.finished_at() {
        Ok(t) => t.to_string(),
        Err(e) => {
            error!("Error {} on getting swap {} finished_at", e, swap.uuid);
            return None;
        },
    };
    let is_success = swap
        .is_success()
        .expect("is_success can return error only when swap is not finished");

    let (maker_coin_ticker, maker_coin_platform) = split_coin(&swap_data.maker_coin);
    let (taker_coin_ticker, taker_coin_platform) = split_coin(&swap_data.taker_coin);

    let params = vec![
        swap_data.maker_coin.clone(),
        maker_coin_ticker,
        maker_coin_platform,
        swap_data.taker_coin.clone(),
        taker_coin_ticker,
        taker_coin_platform,
        swap.uuid.to_string(),
        swap_data.started_at.to_string(),
        finished_at,
        swap_data.maker_amount.to_string(),
        swap_data.taker_amount.to_string(),
        (is_success as u32).to_string(),
    ];
    Some((INSERT_STATS_SWAP, params))
}

fn insert_stats_taker_swap_sql_init(swap: &TakerSavedSwap) -> Option<(&'static str, Vec<String>)> {
    let swap_data = match swap.swap_data() {
        Ok(d) => d,
        Err(e) => {
            error!("Error {} on getting swap {} data", e, swap.uuid);
            return None;
        },
    };
    let finished_at = match swap.finished_at() {
        Ok(t) => t.to_string(),
        Err(e) => {
            error!("Error {} on getting swap {} finished_at", e, swap.uuid);
            return None;
        },
    };
    let is_success = swap
        .is_success()
        .expect("is_success can return error only when swap is not finished");

    let params = vec![
        swap_data.maker_coin.clone(),
        swap_data.taker_coin.clone(),
        swap.uuid.to_string(),
        swap_data.started_at.to_string(),
        finished_at,
        swap_data.maker_amount.to_string(),
        swap_data.taker_amount.to_string(),
        (is_success as u32).to_string(),
    ];
    Some((INSERT_STATS_SWAP_ON_INIT, params))
}

pub fn add_swap_to_index(conn: &Connection, swap: &SavedSwap) {
    let params = vec![swap.uuid().to_string()];
    let query_row = conn.query_row(SELECT_ID_BY_UUID, &params, |row| row.get::<_, i64>(0));
    match query_row.optional() {
        // swap is not indexed yet, go ahead
        Ok(None) => (),
        // swap is already indexed
        Ok(Some(_)) => return,
        Err(e) => {
            error!("Error {} on query {} with params {:?}", e, SELECT_ID_BY_UUID, params);
            return;
        },
    };

    let sql_with_params = match swap {
        SavedSwap::Maker(maker) => insert_stats_maker_swap_sql(maker),
        SavedSwap::Taker(taker) => insert_stats_taker_swap_sql(taker),
    };

    let (sql, params) = match sql_with_params {
        Some(tuple) => tuple,
        None => return,
    };

    debug!("Executing query {} with params {:?}", sql, params);
    if let Err(e) = conn.execute(sql, &params) {
        error!("Error {} on query {} with params {:?}", e, sql, params);
    };
}

pub fn add_and_split_tickers() -> Vec<(&'static str, Vec<String>)> {
    ADD_SPLIT_TICKERS.iter().map(|sql| (*sql, vec![])).collect()
}

#[test]
fn test_split_coin() {
    let input = "";
    let expected = ("".into(), "".into());
    let actual = split_coin(input);
    assert_eq!(expected, actual);

    let input = "RICK";
    let expected = ("RICK".into(), "".into());
    let actual = split_coin(input);
    assert_eq!(expected, actual);

    let input = "RICK-BEP20";
    let expected = ("RICK".into(), "BEP20".into());
    let actual = split_coin(input);
    assert_eq!(expected, actual);

    let input = "RICK-";
    let expected = ("RICK".into(), "".into());
    let actual = split_coin(input);
    assert_eq!(expected, actual);
}
