extern crate alloc;

use alloc::format;
use alloc::string::String;
use embassy_net::tcp::TcpSocket;
use embassy_net::Stack;
use embassy_sync::Duration;
use log::info;

pub struct HttpServer<'a> {
    stack: &'a Stack<'a>,
}

impl<'a> HttpServer<'a> {
    pub fn new(stack: &'a Stack<'a>) -> Self {
        Self { stack }
    }

    pub async fn run(&self) -> ! {
        info!("Starting HTTP server...");

        loop {
            let mut rx_buffer = [0; 1024];
            let mut tx_buffer = [0; 1024];

            // Wait for incoming connections
            let mut socket = TcpSocket::new(self.stack, &mut rx_buffer, &mut tx_buffer);

            if let Err(e) = socket.accept(Duration::from_secs(10)).await {
                info!("Accept error: {:?}", e);
                continue;
            }

            info!("Connection accepted from {:?}", socket.remote_endpoint());

            // Handle the connection
            match self.handle_connection(&mut socket).await {
                Ok(_) => info!("Connection handled successfully"),
                Err(e) => info!("Connection error: {:?}", e),
            }

            socket.close();
        }
    }

    async fn handle_connection(&self, socket: &mut TcpSocket<'a>) -> Result<(), HttpError> {
        let mut buffer = [0; 1024];

        // Read the request
        let len = socket.read(&mut buffer).await?;
        if len == 0 {
            return Err(HttpError::NoData);
        }

        let request =
            core::str::from_utf8(&buffer[..len]).map_err(|_| HttpError::InvalidRequest)?;

        info!("Received request: {}", request.lines().next().unwrap_or(""));

        // Parse the first line to get method and path
        let first_line = request.lines().next().ok_or(HttpError::InvalidRequest)?;
        let mut parts = first_line.split_whitespace();
        let method = parts.next().ok_or(HttpError::InvalidRequest)?;
        let path = parts.next().ok_or(HttpError::InvalidRequest)?;

        // Route the request
        match (method, path) {
            ("GET", "/health") => {
                let response = self.health_response();
                socket.write_all(response.as_bytes()).await?;
            }
            ("GET", "/") => {
                let response = self.root_response();
                socket.write_all(response.as_bytes()).await?;
            }
            _ => {
                let response = self.not_found_response();
                socket.write_all(response.as_bytes()).await?;
            }
        }

        Ok(())
    }

    fn health_response(&self) -> String {
        format!(
            "HTTP/1.1 200 OK\r\n\
             Content-Type: text/plain\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\
             \r\n\
             Wake the f*** up samurai we have beans to burn!",
            "Wake the f*** up samurai we have beans to burn!".len()
        )
    }

    fn root_response(&self) -> String {
        format!(
            "HTTP/1.1 200 OK\r\n\
             Content-Type: text/plain\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\
             \r\n\
             LibreRoaster - Coffee Bean Roaster\r\n\
             \r\n\
             Available endpoints:\r\n\
             GET /health - Health check",
            "LibreRoaster - Coffee Bean Roaster\r\n\r\nAvailable endpoints:\r\nGET /health - Health check".len()
        )
    }

    fn not_found_response(&self) -> String {
        format!(
            "HTTP/1.1 404 Not Found\r\n\
             Content-Type: text/plain\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\
             \r\n\
             Not Found",
            "Not Found".len()
        )
    }
}

#[derive(Debug)]
enum HttpError {
    NoData,
    InvalidRequest,
    ConnectionError,
}

impl core::fmt::Display for HttpError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            HttpError::NoData => write!(f, "No data received"),
            HttpError::InvalidRequest => write!(f, "Invalid HTTP request"),
            HttpError::ConnectionError => write!(f, "Connection error"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_response() {
        let server = HttpServer {
            stack: unsafe { core::mem::zeroed() }, // Not used in test
        };

        let response = server.health_response();
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("Wake the f*** up samurai we have beans to burn!"));
    }

    #[test]
    fn test_not_found_response() {
        let server = HttpServer {
            stack: unsafe { core::mem::zeroed() }, // Not used in test
        };

        let response = server.not_found_response();
        assert!(response.starts_with("HTTP/1.1 404 Not Found"));
    }
}
