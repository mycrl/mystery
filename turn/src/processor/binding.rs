use anyhow::Result;
use bytes::BytesMut;
use faster_stun::{
    Kind,
    Method,
    MessageReader,
    MessageWriter,
};

use super::{
    Context,
    Response,
    SOFTWARE,
};

use faster_stun::attribute::{
    XorMappedAddress,
    MappedAddress,
    ResponseOrigin,
    Software,
};

/// process binding request
///
/// [rfc8489](https://tools.ietf.org/html/rfc8489)
///
/// In the Binding request/response transaction, a Binding request is
/// sent from a STUN client to a STUN server.  When the Binding request
/// arrives at the STUN server, it may have passed through one or more
/// NATs between the STUN client and the STUN server (in Figure 1, there
/// are two such NATs).  As the Binding request message passes through a
/// NAT, the NAT will modify the source transport address (that is, the
/// source IP address and the source port) of the packet.  As a result,
/// the source transport address of the request received by the server
/// will be the public IP address and port created by the NAT closest to
/// the server.  This is called a "reflexive transport address".  The
/// STUN server copies that source transport address into an XOR-MAPPED-
/// ADDRESS attribute in the STUN Binding response and sends the Binding
/// response back to the STUN client.  As this packet passes back through
/// a NAT, the NAT will modify the destination transport address in the
/// IP header, but the transport address in the XOR-MAPPED-ADDRESS
/// attribute within the body of the STUN response will remain untouched.
/// In this way, the client can learn its reflexive transport address
/// allocated by the outermost NAT with respect to the STUN server.
pub fn process<'a>(
    ctx: Context,
    payload: MessageReader,
    w: &'a mut BytesMut,
) -> Result<Response<'a>> {
    let method = Method::Binding(Kind::Response);
    let mut pack = MessageWriter::extend(method, &payload, w);
    pack.append::<XorMappedAddress>(*ctx.addr.as_ref());
    pack.append::<MappedAddress>(*ctx.addr.as_ref());
    pack.append::<ResponseOrigin>(*ctx.external.as_ref());
    pack.append::<Software>(SOFTWARE);
    pack.flush(None)?;
    ctx.observer.binding(&ctx.addr);
    Ok(Some((w, ctx.addr)))
}