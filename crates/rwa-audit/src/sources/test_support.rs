use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread::{self, JoinHandle};

pub(crate) struct MockHttpServer {
    pub(crate) url: String,
    handle: JoinHandle<String>,
}

impl MockHttpServer {
    pub(crate) fn spawn(status: &str, content_type: &str, body: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let status = status.to_string();
        let content_type = content_type.to_string();
        let body = body.to_string();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buffer = [0u8; 4096];
            loop {
                let read = stream.read(&mut buffer).unwrap();
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..read]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }

            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
            String::from_utf8_lossy(&request).into_owned()
        });

        Self {
            url: format!("http://{address}"),
            handle,
        }
    }

    pub(crate) fn request(self) -> String {
        self.handle.join().unwrap()
    }
}
