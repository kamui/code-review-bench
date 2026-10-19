use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Output};
use std::thread::JoinHandle;

/// Answers one connection per response, in order, and returns each request's bytes.
fn serve(responses: Vec<&'static str>) -> (String, JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let server = std::thread::spawn(move || {
        responses
            .into_iter()
            .map(|response| {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = Vec::new();
                let mut buffer = [0; 1024];
                while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                    let read = stream.read(&mut buffer).unwrap();
                    assert!(read > 0, "request ended early");
                    request.extend_from_slice(&buffer[..read]);
                }
                stream.write_all(response.as_bytes()).unwrap();
                String::from_utf8_lossy(&request).into_owned()
            })
            .collect()
    });
    (address, server)
}

fn check(source: &str) -> Output {
    check_with(source, &[])
}

fn check_with(source: &str, environment: &[(&str, &str)]) -> Output {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Command::new(env!("CARGO_BIN_EXE_bench-tools"))
        .env_clear()
        .envs(environment.iter().copied())
        .args(["rates", "check", "--model", "gpt-6.1-sol", "--rates"])
        .arg(repository.join("bench/rates.current.json"))
        .arg("--root")
        .arg(&repository)
        .arg("--source")
        .arg(format!("openai={source}"))
        .output()
        .unwrap()
}

const PAGE: &str =
    "HTTP/1.1 200 OK\r\nContent-Length: 21\r\nConnection: close\r\n\r\n# Not a pricing page\n";

#[test]
fn an_http_page_is_read_after_a_redirect() {
    let (address, server) = serve(vec![
        "HTTP/1.1 302 Found\r\nLocation: /page\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        PAGE,
    ]);
    let output = check(&format!("HTTP://{address}/start"));
    let requests = server.join().unwrap();

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "rates.py: provider pricing section is missing: Standard pricing data\n"
    );
    assert!(requests[0].starts_with("GET /start "), "{requests:?}");
    assert!(requests[1].starts_with("GET /page "), "{requests:?}");
    assert!(
        requests[0]
            .to_ascii_lowercase()
            .contains("user-agent: code-review-bench-rates/1\r\n"),
        "{requests:?}"
    );
}

#[test]
fn an_http_error_status_prints_urllibs_http_error() {
    let (address, server) = serve(vec![
        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    ]);
    let output = check(&format!("http://{address}/"));
    server.join().unwrap();

    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "rates.py: HTTP Error 404: Not Found\n"
    );
}

#[test]
fn a_tls_failure_is_a_source_failure() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.write_all(PAGE.as_bytes()).unwrap();
        let _ = stream.read_to_end(&mut Vec::new());
    });
    let output = check(&format!("https://{address}/"));
    server.join().unwrap();

    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert!(
        output.stderr.starts_with(b"rates.py: <urlopen error "),
        "{output:?}"
    );
}

/// urllib would connect to an https source directly here, because it reads only `https_proxy` for https URLs;
/// rates-decisions.v6.json records that ureq takes the first proxy variable set for every URL.
#[test]
fn an_https_source_tunnels_through_http_proxy() {
    let (address, server) = serve(vec![
        "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    ]);
    let output = check_with(
        "https://pricing.invalid/page.md",
        &[("HTTP_PROXY", &format!("http://{address}"))],
    );
    let requests = server.join().unwrap();

    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(
        requests[0].starts_with("CONNECT pricing.invalid:443 "),
        "{requests:?}"
    );
}
