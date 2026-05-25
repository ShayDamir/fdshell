use crate::shortcstr::{INLINE_MAX, ShortCStr};

impl ShortCStr {
    pub fn verify(&self) -> Result<(), i32> {
        match self {
            ShortCStr::Inline { len, buf } => {
                let n = len.as_u8() as usize;
                if n > INLINE_MAX as usize {
                    return Err(crate::errno::EINVAL);
                }
                if buf[n] != 0 {
                    return Err(crate::errno::EINVAL);
                }
                if buf[..n].contains(&0) {
                    return Err(crate::errno::EINVAL);
                }
                Ok(())
            }
            ShortCStr::Static(s, offset, length) => {
                let full_len = s.count_bytes();
                if offset + length > full_len {
                    return Err(crate::errno::EINVAL);
                }
                let with_nul = s.to_bytes_with_nul();
                if with_nul[offset + length] != 0 {
                    return Err(crate::errno::EINVAL);
                }
                if with_nul[*offset..offset + length].contains(&0) {
                    return Err(crate::errno::EINVAL);
                }
                Ok(())
            }
            ShortCStr::Rc { rc, offset, length } => {
                let full_len = rc.count_bytes();
                if offset + length > full_len {
                    return Err(crate::errno::EINVAL);
                }
                let with_nul = rc.to_bytes_with_nul();
                if with_nul[offset + length] != 0 {
                    return Err(crate::errno::EINVAL);
                }
                if with_nul[*offset..offset + length].contains(&0) {
                    return Err(crate::errno::EINVAL);
                }
                Ok(())
            }
        }
    }
}
