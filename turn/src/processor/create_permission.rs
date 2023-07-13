use anyhow::Result;
use bytes::BytesMut;
use crate::{
    SOFTWARE,
    StunClass,
};

use super::{
    Context,
    Response,
    verify_message,
};

use faster_stun::{
    Kind,
    Method,
    MessageReader,
    MessageWriter,
};

use faster_stun::attribute::{
    ErrKind,
    ErrorCode,
    Error,
    Realm,
    XorPeerAddress,
    Software,
};

use faster_stun::attribute::ErrKind::{
    BadRequest,
    Unauthorized,
    Forbidden,
};

/// return create permission error response
#[inline(always)]
fn reject<'a, 'b, 'c>(
    ctx: Context,
    reader: MessageReader<'a, 'b>,
    bytes: &'c mut BytesMut,
    err: ErrKind,
) -> Result<Option<Response<'c>>> {
    let method = Method::CreatePermission(Kind::Error);
    let mut pack = MessageWriter::extend(method, &reader, bytes);
    pack.append::<ErrorCode>(Error::from(err));
    pack.append::<Realm>(&ctx.env.realm);
    pack.flush(None)?;
    Ok(Some(Response::new(bytes, StunClass::Message, None)))
}

/// return create permission ok response
#[inline(always)]
fn resolve<'a, 'b, 'c>(
    reader: &MessageReader<'a, 'b>,
    key: &[u8; 16],
    bytes: &'c mut BytesMut,
) -> Result<Option<Response<'c>>> {
    let method = Method::CreatePermission(Kind::Response);
    let mut pack = MessageWriter::extend(method, reader, bytes);
    pack.append::<Software>(SOFTWARE);
    pack.flush(Some(key))?;
    Ok(Some(Response::new(bytes, StunClass::Message, None)))
}

/// process create permission request
///
/// [rfc8489](https://tools.ietf.org/html/rfc8489)
///
/// When the server receives the CreatePermission request, it processes
/// as per [Section 5](https://tools.ietf.org/html/rfc8656#section-5)
/// plus the specific rules mentioned here.
///
/// The message is checked for validity.  The CreatePermission request
/// MUST contain at least one XOR-PEER-ADDRESS attribute and MAY contain
/// multiple such attributes.  If no such attribute exists, or if any of
/// these attributes are invalid, then a 400 (Bad Request) error is
/// returned.  If the request is valid, but the server is unable to
/// satisfy the request due to some capacity limit or similar, then a 508
/// (Insufficient Capacity) error is returned.
///
/// If an XOR-PEER-ADDRESS attribute contains an address of an address
/// family that is not the same as that of a relayed transport address
/// for the allocation, the server MUST generate an error response with
/// the 443 (Peer Address Family Mismatch) response code.
///
/// The server MAY impose restrictions on the IP address allowed in the
/// XOR-PEER-ADDRESS attribute; if a value is not allowed, the server
/// rejects the request with a 403 (Forbidden) error.
///
/// If the message is valid and the server is capable of carrying out the
/// request, then the server installs or refreshes a permission for the
/// IP address contained in each XOR-PEER-ADDRESS attribute as described
/// in [Section 9](https://tools.ietf.org/html/rfc8656#section-9).  
/// The port portion of each attribute is ignored and may be any arbitrary
/// value.
///
/// The server then responds with a CreatePermission success response.
/// There are no mandatory attributes in the success response.
///
/// > NOTE: A server need not do anything special to implement
/// idempotency of CreatePermission requests over UDP using the
/// "stateless stack approach".  Retransmitted CreatePermission
/// requests will simply refresh the permissions.
pub async fn process<'a, 'b, 'c>(
    ctx: Context,
    reader: MessageReader<'a, 'b>,
    bytes: &'c mut BytesMut,
) -> Result<Option<Response<'c>>> {
    let peer = match reader.get::<XorPeerAddress>() {
        None => return reject(ctx, reader, bytes, BadRequest),
        Some(a) => a,
    };

    if ctx.env.external.ip() != peer.ip() {
        return reject(ctx, reader, bytes, Forbidden);
    }

    let (username, key) = match verify_message(&ctx, &reader).await {
        None => return reject(ctx, reader, bytes, Unauthorized),
        Some(ret) => ret,
    };

    if ctx
        .env
        .router
        .bind_port(&ctx.addr, peer.port(), None)
        .is_none()
    {
        return reject(ctx, reader, bytes, Forbidden);
    }

    ctx.env
        .observer
        .create_permission(&ctx.addr, username, &peer);
    resolve(&reader, &key, bytes)
}
