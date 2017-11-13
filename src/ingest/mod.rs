mod parse;
mod persist;
mod pvm_cache;

use std::collections::HashMap;
use std::io::BufRead;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

use rayon::prelude::*;
use neo4j::cypher::CypherStream;
use serde_json;

use self::pvm_cache::PVMCache;
use trace::TraceEvent;

fn print_time(ref tmr: Instant) {
    let dur = tmr.elapsed();
    println!("{:.3} Seconds elapsed",
             dur.as_secs() as f64
             + dur.subsec_nanos() as f64 * 1e-9);
}

pub fn ingest<R>(stream: R, mut db: CypherStream)
where
    R: BufRead,
{
    let tmr = Instant::now();
    db.run_unchecked("CREATE INDEX ON :Process(db_id)", HashMap::new());

    const BATCH_SIZE: usize = 0x80000;

    let mut cache = PVMCache::new();

    let (mut send, recv) = mpsc::sync_channel(BATCH_SIZE*2);

    let db_worker = thread::spawn(move || { persist::execute_loop(db, recv); });

    let mut pre_vec: Vec<String> = Vec::with_capacity(BATCH_SIZE);
    let mut post_vec: Vec<Option<TraceEvent>> = Vec::with_capacity(BATCH_SIZE);
    let mut lines = stream.lines();

    loop {
        pre_vec.clear();
        while pre_vec.len() < BATCH_SIZE {
            let l = match lines.next() {
                Some(l) => {
                    match l {
                        Ok(l) => l,
                        Err(perr) => {
                            println!("Parsing error: {}", perr);
                            continue;
                        }
                    }
                }
                None => {
                    break;
                }
            };
            if l.is_empty() {
                continue;
            }
            pre_vec.push(l);
        }

        pre_vec
            .par_iter()
            .map(|s| {
                match serde_json::from_slice(s.as_bytes()) {
                    Ok(evt) => Some(evt),
                    Err(perr) => {
                        println!("Parsing error: {}", perr);
                        println!("{}", s);
                        None
                    }
                }
            })
            .collect_into(&mut post_vec);

        for tr in post_vec.drain(..) {
            match tr {
                Some(tr) => {
                    if let Err(perr) = parse::parse_trace(&tr, &mut send, &mut cache) {
                        println!("PVM parsing error {}", perr);
                    }
                }
                None => continue,
            }
        }
        if pre_vec.len() < BATCH_SIZE {
            break;
        }
    }
    drop(send);
    println!("Parse Complete");
    print_time(tmr);
    if let Err(e) = db_worker.join() {
        println!("Database thread panicked: {:?}", e);
    }
    println!("Ingestion Complete");
    print_time(tmr);
}