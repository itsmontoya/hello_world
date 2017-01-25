extern crate env_logger;
extern crate futures;
extern crate hyper;
extern crate serde_json;
extern crate futures_cpupool;

use std::io;
use std::result;
use std::{thread, time};

use hyper::header::{ContentLength, ContentType};
use hyper::server::{Http, Service, Request, Response};
use hyper::mime::{Mime, TopLevel, SubLevel};

use futures::future;
use futures::Stream;
use futures::Future;
use futures_cpupool::CpuPool;


fn main() {
    drop(env_logger::init());

    let pool = CpuPool::new(4);
    let addr = "127.0.0.1:8080".parse().unwrap();
    let server = Http::new()
        .bind(&addr, move || {
            let pc = pool.clone();
            Ok(Responder { pool: pc })
        })
        .unwrap();

    println!("Listening on http://{}", server.local_addr().unwrap());
    server.run().unwrap();
}



struct Responder {
    pool: CpuPool,
}

type ResponseFuture = Box<Future<Item = Response, Error = hyper::Error>>;

impl Service for Responder {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = ResponseFuture;

    fn call(&self, _request: Request) -> Self::Future {
        match _request.path() {
            _ => self.handle_service(_request),
        }
    }
}

impl Responder {
    fn handle_service(&self, req: Request) -> ResponseFuture {
        let body = req.body();
        let body_vec = Vec::new();

        Box::new(body.fold(body_vec, |mut acc, chunk| {
                acc.extend_from_slice(chunk.as_ref());
                Ok::<Vec<u8>, hyper::Error>(acc)
            })
            .and_then(move |body_vec| {
                // I know I'm using tons of unwrap here, I'll get proper error handling soon
                let body_str = String::from_utf8(body_vec).unwrap();

                // Sleep for 3 seconds (to simulate a slow db request)
                self.pool.spawn_fn(|| {
                    thread::sleep(time::Duration::from_millis(3000));
                    Ok(())
                });

                Ok(body_str)
            }))
    }
}
