use sys::fd::{close, dup2};
use sys::rw::{read, write};
use sys::net::socketpair;
use sys::shellfd::{recv_fd, send_fd, SHELLFD, TAG_MAX};

#[test]
fn test_send_recv_fd() -> Result<(), i32> {
    let mut pair = [0; 2];
    socketpair(&mut pair)?;

    if pair[0] != SHELLFD {
        dup2(pair[0], SHELLFD)?;
        let _ = close(pair[0]);
    }
    let receiver = pair[1];

    let mut test_pair = [0; 2];
    socketpair(&mut test_pair)?;

    send_fd(test_pair[0], c"test")?;
    let _ = close(test_pair[0]);
    write(test_pair[1], b"42")?;
    let _ = close(test_pair[1]);

    let mut tag = [0u8; TAG_MAX];
    let (test_fd, _tag) = recv_fd(receiver, &mut tag)?;

    let mut buf = [0u8; 8];
    assert_eq!(read(test_fd, &mut buf)?, 2);
    assert_eq!(&buf[..2], b"42");
    assert_eq!(read(test_fd, &mut buf)?, 0);

    let _ = close(test_fd);
    let _ = close(receiver);
    Ok(())
}
