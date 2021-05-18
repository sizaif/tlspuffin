use std::io;
use std::io::{Read, Write};

use openssl::ssl::SslStream;
use rustls::internal::msgs::codec::Codec;
use rustls::internal::msgs::deframer::MessageDeframer;

#[allow(unused)] // used in docs
use crate::agent::Agent;
use crate::openssl_binding;
use rustls::internal::msgs::message::Message;
use crate::debug::debug_binary_message_with_info;

pub trait Stream: std::io::Read + std::io::Write {
    fn add_to_inbound(&mut self, data: &Message);
    fn add_to_outbound(&mut self, data: &Message, prepend: bool);
    /// Takes a single TLS message from the outbound channel in binary
    fn take_message_from_outbound(&mut self) -> Option<Message>;
    // Gets a TLS message from the outbound channel and does NOT remove the content
    fn peek_message_from_outbound(&mut self) -> Option<Message>;
    /// Takes the whole content of the outbound channel; after this call the outbound
    /// channel is empty
    fn take_from_inbound(&mut self) -> Option<Message>;

    fn describe_state(&self) -> &'static str;
}

/// Describes in- or outbound channels of an [`Agent`]. Each [`Agent`] can send and receive data.
/// This is modeled by two separate Channels in [`MemoryStream`]. Internally a Channel is just an
/// in-memory seekable buffer.
pub type Channel = io::Cursor<Vec<u8>>;

/// A MemoryStream has two [`Channel`]s. The Stream also implements the [`Write`] and [`Read`] trait.
/// * When writing to a MemoryStream its outbound channel gets filled.
/// * When reading from a MemoryStream data is taken from the inbound channel.
///
/// This makes it possible for an Agent to treat a [`MemoryStream`] like a TLS socket! By writing
/// to this socket you are sending data out. By reading from it you receive data.
///
/// **Note: There need to be two separate buffer! Else for example a TLS socket would read and write
/// into the same buffer**
pub struct MemoryStream {
    inbound: Channel,
    outbound: Channel,
}

/// A MemoryStream which wraps an SslStream.
pub struct OpenSSLStream {
    openssl_stream: SslStream<MemoryStream>,
    server: bool,
}

impl Stream for OpenSSLStream {
    fn add_to_inbound(&mut self, data: &Message) {
        self.openssl_stream.get_mut().add_to_inbound(data)
    }

    fn add_to_outbound(&mut self, data: &Message, prepend: bool) {
        self.openssl_stream.get_mut().add_to_outbound(data, prepend)
    }

    fn take_message_from_outbound(&mut self) -> Option<Message> {
        self.openssl_stream.get_mut().take_message_from_outbound()
    }

    fn peek_message_from_outbound(&mut self) -> Option<Message> {
        self.openssl_stream.get_mut().peek_message_from_outbound()
    }

    fn take_from_inbound(&mut self) -> Option<Message> {
        let openssl_stream = &mut self.openssl_stream;
        if self.server {
            openssl_binding::server_accept(openssl_stream)
        } else {
            openssl_binding::client_connect(openssl_stream)
        }
    }

    fn describe_state(&self) -> &'static str {
        // Very useful for nonblocking according to docs:
        // https://www.openssl.org/docs/manmaster/man3/SSL_state_string.html
        // When using nonblocking sockets, the function call performing the handshake may return
        // with SSL_ERROR_WANT_READ or SSL_ERROR_WANT_WRITE condition,
        // so that SSL_state_string[_long]() may be called.
        self.openssl_stream.ssl().state_string_long()
    }
}

impl Read for OpenSSLStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.openssl_stream.get_mut().read(buf)
    }
}

impl Write for OpenSSLStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.openssl_stream.get_mut().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.openssl_stream.get_mut().flush()
    }
}

impl OpenSSLStream {
    pub fn new(server: bool) -> Self {
        let memory_stream = MemoryStream::new();
        OpenSSLStream {
            openssl_stream: if server {
                let (cert, pkey) = openssl_binding::generate_cert();
                openssl_binding::create_openssl_server(memory_stream, &cert, &pkey)
            } else {
                openssl_binding::create_openssl_client(memory_stream)
            },
            server,
        }
    }
}

impl MemoryStream {
    pub fn new() -> Self {
        Self {
            inbound: io::Cursor::new(Vec::new()),
            outbound: io::Cursor::new(Vec::new()),
        }
    }
}

impl Stream for MemoryStream {
    fn add_to_inbound(&mut self, message: &Message) {
        let mut out: Vec<u8> = Vec::new();
        message.encode(&mut out);
        self.inbound.get_mut().extend_from_slice(&out);
    }

    fn add_to_outbound(&mut self, message: &Message, prepend: bool) {
        let mut out: Vec<u8> = Vec::new();
        message.encode(&mut out);

        if prepend {
            for datum in out.iter().rev() {
                self.outbound.get_mut().insert(0, *datum);
            }
        } else {
            self.outbound.get_mut().extend_from_slice(out.as_slice());
        }
    }

    fn take_message_from_outbound(&mut self) -> Option<Message> {
        let mut deframer = MessageDeframer::new();
        if let Ok(_) = deframer.read(&mut self.outbound.get_ref().as_slice()) {
            let mut rest_buffer: Vec<u8> = Vec::new();

            let mut first_message = deframer.frames.pop_front().unwrap();

            for message in deframer.frames {
                message.encode(&mut rest_buffer);
            }

            self.outbound.set_position(0);
            self.outbound.get_mut().clear();
            self.outbound.write_all(&rest_buffer).unwrap();

            first_message.decode_payload();

            return Some(first_message);
        } else {
            None
        }
    }

    fn peek_message_from_outbound(&mut self) -> Option<Message> {
        let mut deframer = MessageDeframer::new();
        if let Ok(_) = deframer.read(&mut self.outbound.get_ref().as_slice()) {
            let mut first_message = deframer.frames.pop_front().unwrap();

            first_message.decode_payload();
            return Some(first_message);
        } else {
            None
        }
    }

    fn take_from_inbound(&mut self) -> Option<Message> {
        let buffer = self.inbound.get_ref().clone();
        self.inbound.get_mut().clear();
        self.inbound.set_position(0);

        let mut deframer = MessageDeframer::new();
        if let Ok(size) = deframer.read(&mut buffer.as_slice()) {
            info!("{}", size)
        }
        debug_binary_message_with_info("Received", &buffer);

        let message = Message::read_bytes(&buffer);
        if let Some(mut message) = message {
            message.decode_payload();
            return Some(message);
        }
        return message;
    }

    fn describe_state(&self) -> &'static str {
        todo!()
    }
}

impl Read for MemoryStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inbound.read(buf)?;

        // Clear as soon as we read all data
        if self.inbound.position() == self.inbound.get_ref().len() as u64 {
            self.inbound.set_position(0);
            self.inbound.get_mut().clear();
        }
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "no data available",
            ));
        }
        Ok(n)
    }
}

impl Write for MemoryStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.outbound.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
