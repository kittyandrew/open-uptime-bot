use lazy_static::lazy_static;
use prometheus::{self, Encoder, HistogramVec, IntCounter, IntCounterVec, TextEncoder};
use rocket::fairing::{Fairing, Info, Kind};
use rocket::{Data, Request, Response, State};
use std::time::Instant;

lazy_static! {
    pub static ref TOTAL_REQUESTS_SERVED: IntCounter = prometheus::register_int_counter!(
        "total_requests_served",
        "Total number of requests served (including rejected)"
    )
    .unwrap();
    pub static ref ENDPOINTS_REQUESTS_SERVED: IntCounterVec = prometheus::register_int_counter_vec!(
        "endpoints_requests_served",
        "Total number of requests served per endpoint",
        &["method", "endpoint"]
    )
    .unwrap();
    pub static ref ENDPOINT_REQUESTS_DURATION: HistogramVec = prometheus::register_histogram_vec!(
        "endpoint_requests_duration_seconds",
        "Endpoint requests handling latencies in in seconds",
        &["method", "endpoint"]
    )
    .unwrap();
}

#[derive(Copy, Clone, Debug)]
struct DurationTimer(Option<Instant>);

pub struct PrometheusCollection;

#[rocket::async_trait]
impl Fairing for PrometheusCollection {
    fn info(&self) -> Info {
        Info {
            name: "Prometheus metrics",
            kind: Kind::Request | Kind::Response,
        }
    }

    async fn on_request(&self, request: &mut Request<'_>, _: &mut Data<'_>) {
        // Note(andrew): Setting timer value here to the local request cache, since this
        //     is executed prior to entering our endpoints, which means we will be able
        //     to time request latency.
        request.local_cache(|| DurationTimer(Some(Instant::now())));
    }

    async fn on_response<'r>(&self, request: &'r Request<'_>, response: &mut Response<'r>) {
        let method = request.method().to_string();
        let endpoint = request.uri().path().to_string();
        if endpoint != "/api/v1/metrics" {
            // Record total request count.
            TOTAL_REQUESTS_SERVED.inc();

            // Measuring request latency (internal processing time). Doing this first, so any
            // code below adds minimal overhead to the processing time. Just make sure that the
            // code below actually isn't artificially slow.
            let duration_timer = request.local_cache(|| DurationTimer(None));
            if let Some(duration) = duration_timer.0.map(|st| st.elapsed()) {
                ENDPOINTS_REQUESTS_SERVED
                    .local()
                    .with_label_values(&[&method, &endpoint])
                    .inc();

                let latency_ms = duration.as_micros() as f64 / 1000.;
                ENDPOINT_REQUESTS_DURATION
                    .local()
                    .with_label_values(&[&method, &endpoint])
                    .observe(latency_ms / 1000.);

                // While we can, lets add response header with timing as well.
                response.set_raw_header("X-Response-Time", format!("{latency_ms} ms"));
            };
        }
    }
}

// Native metrics export support for Prometheus.
#[get("/api/v1/metrics")]
pub async fn get_metrics(encoder: &State<TextEncoder>) -> String {
    let mut buffer = Vec::new();
    encoder.encode(&prometheus::gather(), &mut buffer).unwrap();
    return String::from_utf8(buffer).unwrap();
}
