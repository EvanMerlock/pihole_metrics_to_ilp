#[macro_use] extern crate rocket;

use std::{fs::File, io::{Read, Write}};

use rocket::http::{Status, ContentType};
use rusqlite::Connection;

const SQLLITE_DB_LOCATION: &'static str = "./pihole-FTL.db";
const TIME_CHECK_NAME: &'static str = "./last_time_check";
const LINE_DELIMITER: &'static str = "\n";
const SQL_QUERY: &'static str = 
    "SELECT 
        timestamp||substr(id,length(id)-2,3),
        type,
        status,
        domain,
        client,
        IFNULL(forward, \'null\') forward,
        id 
    FROM 
        queries 
    WHERE 
        id > ?1";

#[derive(Debug)]
pub struct PiholeEntry {

    id: u64,
    time: u64,
    query_type: u64,
    domain: String,
    client: String,
    status: u64,
    upstream: String

}

#[launch]
fn rocket() -> _ {
    rocket::build().mount("/", routes![get_pihole_stats])
}

#[get("/")]
fn get_pihole_stats() -> (Status, (ContentType, String)) {
    let timecheck_file = File::open(TIME_CHECK_NAME);
    let mut last_id: u64 = 0;

    if timecheck_file.is_ok() {
        let mut id_buf = String::new();
        match timecheck_file.expect("wowie something really fucked up").read_to_string(&mut id_buf) {
            Ok(_) => {},
            Err(e) => return (Status::InternalServerError, (ContentType::Text, e.to_string())),
        };
        last_id = match id_buf.parse() {
            Ok(item) => item,
            Err(e) => return (Status::InternalServerError, (ContentType::Text, e.to_string())),
        };
    }

    let sqllite_connection = match Connection::open(SQLLITE_DB_LOCATION) {
        Ok(db_conn) => db_conn,
        Err(e) => return (Status::InternalServerError, (ContentType::Text, e.to_string())),
    };

    let mut stmt = match sqllite_connection.prepare(SQL_QUERY) {
        Ok(results) => results,
        Err(e) => return (Status::InternalServerError, (ContentType::Text, e.to_string())),
    };

    let pihole_entries = match stmt.query_map([last_id], |row| {
        let res = process_individual_value(row)?;
        Ok(res)
    }) {
        Ok(r) => r,
        Err(e) => return (Status::InternalServerError, (ContentType::Text, e.to_string())),
    };

    let mut buf = String::new();
    let mut next = last_id;

    for item in pihole_entries {
        let pre_ilp = match item {
            Ok(i) => i,
            Err(e) => return (Status::InternalServerError, (ContentType::Text, e.to_string())),
        };
        let (ilp, id) = pihole_entry_to_ilp(pre_ilp);
        buf.push_str(&ilp);
        buf.push_str(LINE_DELIMITER);

        if next <= id {
            next = id;
        } 
    }

    let mut new_timecheck_file = match File::create(TIME_CHECK_NAME) {
        Ok(file) => file,
        Err(e) => return (Status::InternalServerError, (ContentType::Text, e.to_string())),
    };
    let id_str = format!("{}", next);
    match new_timecheck_file.write_all(id_str.as_bytes()) {
        Ok(_) => {},
        Err(e) => return (Status::InternalServerError, (ContentType::Text, e.to_string()))
    };

    (Status::Ok, (ContentType::Text, buf))
}

fn process_individual_value(row: &rusqlite::Row) -> Result<PiholeEntry, rusqlite::Error> {


    let pre_time: String = row.get(0)?;

    Ok(PiholeEntry {
        time: pre_time.parse().expect("oops shit broke"),
        query_type: row.get(1)?,
        status: row.get(2)?,
        domain: row.get(3)?,
        client: row.get(4)?,
        upstream: row.get(5)?,
        id: row.get(6)?
    })

}

fn pihole_entry_to_ilp(pihole_entry: PiholeEntry) -> (String, u64) {

    (format!("dnsquery,query_type=\"{}\",client=\"{}\",status=\"{}\" domain=\"{}\",upstream=\"{}\" {}", 
        pihole_entry.query_type, 
        pihole_entry.client, 
        pihole_entry.status, 
        pihole_entry.domain, 
        pihole_entry.upstream, 
        pihole_entry.time
    ), pihole_entry.id)

}