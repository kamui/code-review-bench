use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;

fn files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut found = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                found.insert(path.clone(), std::fs::read(&path).unwrap());
            }
        }
    }
    found
}

#[test]
fn rates_check_ignores_a_curlrc_that_names_an_output_file() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let scratch = std::env::temp_dir().join(format!(
        "bench-tools-curlrc-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let schema = scratch.join("root/bench/schema");
    let curl_home = scratch.join("curl-home");
    std::fs::create_dir_all(&schema).unwrap();
    std::fs::create_dir_all(&curl_home).unwrap();
    std::fs::create_dir_all(scratch.join("home")).unwrap();
    std::fs::copy(
        repository.join("bench/schema/rates.schema.json"),
        schema.join("rates.schema.json"),
    )
    .unwrap();
    let catalog = scratch.join("catalog.json");
    std::fs::copy(repository.join("bench/rates.current.json"), &catalog).unwrap();
    std::fs::write(
        curl_home.join(".curlrc"),
        format!("output = \"{}\"\n", catalog.display()),
    )
    .unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut buffer = [0; 1024];
        while !request.windows(4).any(|window| window == b"\r\n\r\n") {
            let read = stream.read(&mut buffer).unwrap();
            assert!(read > 0, "request ended early");
            request.extend_from_slice(&buffer[..read]);
        }
        let body = "# Not a pricing page\n";
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
    });

    let before = files(&scratch);
    let output = Command::new(env!("CARGO_BIN_EXE_bench-tools"))
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", scratch.join("home"))
        .env("CURL_HOME", &curl_home)
        .args(["rates", "check", "--model", "gpt-6.1-sol", "--rates"])
        .arg(&catalog)
        .arg("--root")
        .arg(scratch.join("root"))
        .arg("--source")
        .arg(format!("openai=http://{address}/"))
        .output()
        .unwrap();
    server.join().unwrap();
    let after = files(&scratch);
    std::fs::remove_dir_all(&scratch).unwrap();

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Standard pricing"),
        "{output:?}"
    );
    assert!(
        before == after,
        "rates check changed files under {scratch:?}"
    );
}
