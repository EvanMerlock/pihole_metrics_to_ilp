#[macro_use] extern crate rocket;

use std::{fs::File, io::{Read, Write}};

use rocket::{http::{Status, ContentType}, State};
use rusqlite::Connection;
use serde::Deserialize;

const SQLLITE_DB_LOCATION: &'static str = "/pihole-FTL.db";
const LOCK_FILE: &'static str = "/last_file_check";
const LINE_DELIMITER: &'static str = "\n";
const SQL_QUERY: &'static str = 
    "SELECT 
        timestamp,
        type,
        status,
        domain,
        client,
        IFNULL(forward, \'null\') forward,
        reply_type,
        id
    FROM 
        queries 
    WHERE 
        id > ?1
    ORDER BY
        id
    LIMIT ?2";

#[derive(Debug)]
pub struct PiholeEntry {

    id: u64,
    time: u64,
    query_type: u64,
    domain: String,
    client: String,
    status: u64,
    upstream: String,
    reply_type: u64,
}

#[derive(Deserialize, Debug)]
pub struct EnvConfig {
    #[serde(default="get_default_limit")]
    limit: u64,
    #[serde(default="get_default_db_location")]
    db_location: String,
    #[serde(default="get_default_lock_file_location")]
    lock_file: String,
}

fn get_default_limit() -> u64 {
    5000
}

fn get_default_db_location() -> String {
    SQLLITE_DB_LOCATION.to_string()
}

fn get_default_lock_file_location() -> String {
    LOCK_FILE.to_string()
}

#[launch]
fn rocket() -> _ {
    let cfg = match envy::prefixed("PIHOLEM_").from_env::<EnvConfig>() {
        Ok(config) => config,
        Err(error) => panic!("{:#?}", error)
    };
    rocket
        ::build()
        .mount("/", routes![get_pihole_stats])
        .manage(cfg)
}

#[get("/")]
fn get_pihole_stats(config: &State<EnvConfig>) -> (Status, (ContentType, String)) {
    let lock_file = File::open(config.lock_file.clone());
    let mut last_id: u64 = 0;

    if lock_file.is_ok() {
        let mut id_buf = String::new();
        match lock_file.expect("Could not open checking file").read_to_string(&mut id_buf) {
            Ok(_) => {},
            Err(e) => return (
                Status::InternalServerError, 
                (ContentType::Text, format_error("Failed to read lock file", e.to_string()))
            ),
        };
        last_id = match id_buf.trim().parse() {
            Ok(item) => item,
            Err(e) => return (
                Status::InternalServerError, (ContentType::Text, format_error("Failed to parse last ID", e.to_string()))
            ),
        };
    }

    let sqllite_connection = match Connection::open(config.db_location.clone()) {
        Ok(db_conn) => db_conn,
        Err(e) => return (
            Status::InternalServerError, (ContentType::Text, format_error("Failed to open SQLLite connection", e.to_string()))
        ),
    };

    let mut stmt = match sqllite_connection.prepare(SQL_QUERY) {
        Ok(results) => results,
        Err(e) => return (
            Status::InternalServerError, (ContentType::Text, format_error("Failed to prepare SQL statement", e.to_string()))
        ),
    };

    let pihole_entries = match stmt.query_map([last_id, config.limit], |row| {
        let res = process_individual_value(row)?;
        Ok(res)
    }) {
        Ok(r) => r,
        Err(e) => return (
            Status::InternalServerError, (ContentType::Text, format_error("Failed to execute query", e.to_string()))
        ),
    };

    let mut buf = String::new();
    let mut next = last_id;

    for item in pihole_entries {
        let pre_ilp = match item {
            Ok(i) => i,
            Err(e) => return (
                Status::InternalServerError, (ContentType::Text, format_error("Failed to get entry from row", e.to_string()))
            ),
        };
        let (ilp, id) = pihole_entry_to_ilp(pre_ilp);
        buf.push_str(&ilp);
        buf.push_str(LINE_DELIMITER);

        if next <= id {
            next = id;
        } 
    }

    let mut new_lock_file = match File::create(config.lock_file.clone()) {
        Ok(file) => file,
        Err(e) => return (
            Status::InternalServerError, (ContentType::Text, format_error("Failed to lock lockfile", e.to_string()))
        ),
    };
    let id_str = format!("{}", next);
    match new_lock_file.write_all(id_str.as_bytes()) {
        Ok(_) => {},
        Err(e) => return (
            Status::InternalServerError, (ContentType::Text, format_error("Failed to write to lockfile", e.to_string()))
        )
    };

    (Status::Ok, (ContentType::Text, buf))
}

fn format_error(hdr: &'static str, msg: String) -> String {
    format!("{}: {}", hdr, msg)
}

fn process_individual_value(row: &rusqlite::Row) -> Result<PiholeEntry, rusqlite::Error> {

    let id = row.get(7)?;

    let pre_time: u64 = row.get(0)?;
    let (hundreds, tens, ones) = extract_places(id);
    let real_time: String = format!("{}{}{}{}", pre_time, hundreds, tens, ones);

    Ok(PiholeEntry {
        time: real_time.parse().expect("oops shit broke"),
        query_type: row.get(1)?,
        status: row.get(2)?,
        domain: row.get(3)?,
        client: row.get(4)?,
        upstream: row.get(5)?,
        reply_type: row.get(6)?,
        id,
    })

}

fn extract_places(id: u64) -> (u64, u64, u64) {
    let hundreds_place = (id / 100) % 10;
    let tens_place = (id / 10) % 10;
    let ones_place = id % 10;
    (hundreds_place, tens_place, ones_place)
}

fn pihole_entry_to_ilp(pihole_entry: PiholeEntry) -> (String, u64) {

    (format!("{},{},{},{},{},{},{},{}", 
        pihole_entry.id,
        pihole_entry.time,
        pihole_entry.query_type,
        pihole_entry.status,
        pihole_entry.reply_type,
        pihole_entry.client, 
        pihole_entry.domain,
        pihole_entry.upstream,
    ), pihole_entry.id)

}
