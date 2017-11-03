#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }

}

extern crate bytes;
#[macro_use]
extern crate prost_derive;
extern crate prost;
extern crate tokio_proto;
extern crate tokio_io;

pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/messages.rs"));

    use tokio_io::{AsyncRead, AsyncWrite};
    use tokio_io::codec::Framed;
    use prost::Message;
    use tokio_proto::pipeline::ServerProto;
    use tokio_proto::pipeline::ClientProto;
    use tokio_io::codec::{Encoder, Decoder};
    use bytes::BytesMut;
    use std::io;

    pub struct ProtoCodec;
    pub struct ProtoClient;
    pub struct ProtoProto;

    impl Decoder for ProtoClient {
        type Item = ToClient;
        type Error = io::Error;

        fn decode(&mut self, buf: &mut BytesMut) -> io::Result<Option<ToClient>> {
            let cbuf = buf.clone();
            // Hack - decode doesnt seem to
            // return an error if buf i empty
            if buf.len() == 0 {
                return Ok(None);
            }
            let msg = match ToClient::decode(cbuf) {
                Err(_) => return Ok(None),
                Ok(msg) => msg,
            };
            buf.clear();
            Ok(Some(msg))
        }
    }

    impl Encoder for ProtoClient {
        type Item = FromClient;
        type Error = io::Error;

        fn encode(&mut self, msg: Self::Item, buf: &mut BytesMut) -> io::Result<()> {
            if let Err(err) = msg.encode(buf) {
                return {
                    Err(io::Error::new(io::ErrorKind::InvalidData, err.to_string()))
                };
            }
            Ok(())
        }
    }

    impl Decoder for ProtoCodec {
        type Item = FromClient;
        type Error = io::Error;

        fn decode(&mut self, buf: &mut BytesMut) -> io::Result<Option<FromClient>> {
            let cbuf = buf.clone();
            // Hack - decode doesnt seem to
            // return an error if buf i empty
            if buf.len() == 0 {
                return Ok(None);
            }
            let msg = match FromClient::decode(cbuf) {
                Err(_) => return Ok(None),
                Ok(msg) => msg,
            };
            buf.clear();
            Ok(Some(msg))
        }
    }

    impl Encoder for ProtoCodec {
        type Item = ToClient;
        type Error = io::Error;

        fn encode(&mut self, msg: Self::Item, buf: &mut BytesMut) -> io::Result<()> {
            if let Err(err) = msg.encode(buf) {
                return {
                    Err(io::Error::new(io::ErrorKind::InvalidData, err.to_string()))
                };
            }
            Ok(())
        }
    }

    impl<T: AsyncRead + AsyncWrite + 'static> ServerProto<T> for ProtoProto {
        type Request = FromClient;
        type Response = ToClient;

        type Transport = Framed<T, ProtoCodec>;
        type BindTransport = Result<Self::Transport, io::Error>;
        fn bind_transport(&self, io: T) -> Self::BindTransport {
            Ok(io.framed(ProtoCodec))
        }
    }

    impl<T: AsyncRead + AsyncWrite + 'static> ClientProto<T> for ProtoProto {
        type Request = ToClient;
        type Response = FromClient;

        type Transport = Framed<T, ProtoCodec>;
        type BindTransport = Result<Self::Transport, io::Error>;

        fn bind_transport(&self, io: T) -> Self::BindTransport {
            Ok(io.framed(ProtoCodec))
        }
    }
}
