use nix::errno::Errno;
use nix::sys::ptrace;
use nix::sys::ptrace::AddressType;
use nix::unistd::Pid;

const SIZE_OF_PTR: usize = std::mem::size_of::<usize>();

// retrieve arbitrary-length data
pub fn read_data(pid: Pid, addr: AddressType, len: usize) -> Result<Vec<u8>, Errno> {
    let mut data = vec![0u8; len];
    let mut i = 0;
    // aligned read,reference:https://cs.opensource.google/go/go/+/refs/tags/go1.23.3:src/syscall/syscall_linux.go;l=845
    let align_offset = addr as usize % SIZE_OF_PTR;
    if align_offset != 0 {
        let res = ptrace::read(pid, addr.wrapping_byte_sub(align_offset));
        match res {
            Ok(val) => {
                i += copy(&mut data, i, len - i, &val.to_ne_bytes()[align_offset..]);
            }
            Err(e) => return Err(e),
        }
    }
    while i < len {
        let res = ptrace::read(pid, addr.wrapping_byte_add(i));
        match res {
            Ok(val) => {
                i += copy(&mut data, i, len - i, &val.to_ne_bytes());
            }
            Err(e) => return Err(e),
        }
    }
    Ok(data)
}

pub fn write_data(pid: Pid, addr: AddressType, data: &[u8]) {
    // todo: aligned write
    let len_in_word = data.len() / SIZE_OF_PTR;
    for i in 0..len_in_word {
        let val = unsafe {
            std::mem::transmute::<[u8; SIZE_OF_PTR], i64>(
                data[i * SIZE_OF_PTR..(i + 1) * SIZE_OF_PTR]
                    .try_into()
                    .unwrap(),
            )
        };
        ptrace::write(pid, addr.wrapping_byte_add(i*SIZE_OF_PTR), val).unwrap();
    }
    let remainder = data.len() % SIZE_OF_PTR;
    if remainder > 0 {
        panic!("unaligned write");
        // let val = unsafe {
        //     std::mem::transmute::<[u8; SIZE_OF_PTR], i64>(
        //         data[len_in_word * SIZE_OF_PTR..len_in_word * SIZE_OF_PTR + remainder]
        //             .try_into()
        //             .unwrap(),
        //     )
        // };
        // ptrace::write(pid,addr.wrapping_byte_add(len_in_word),val).unwrap();
    }
}

// 确保拷贝的字节数不超过data的长度,返回拷贝的字节数
fn copy(data: &mut [u8], i: usize, bytes_to_be_copied: usize, src: &[u8]) -> usize {
    if bytes_to_be_copied < src.len() {
        data[i..i + bytes_to_be_copied].copy_from_slice(&src[0..bytes_to_be_copied]);
        bytes_to_be_copied
    } else {
        data[i..i + src.len()].copy_from_slice(src);
        src.len()
    }
}
