use std::ffi::CString;
use std::ptr;

fn main() {
    let mut args = std::env::args();

    let _ = args.next().unwrap();
    let image_fs = args.next().unwrap(); // 镜像文件系统，提前解压
    let command = args.next().unwrap(); // 要运行的容器命令

    // 运行容器命令的参数列表
    let mut command_args: Vec<CString> = args.map(|arg| CString::new(arg).unwrap()).collect();
    command_args.insert(0, CString::new(command.as_str()).unwrap());
    let mut command_args: Vec<*const libc::c_char> =
        command_args.iter().map(|arg| arg.as_ptr()).collect();
    command_args.push(ptr::null());

    // 转换成绝对目录
    let image_fs = std::path::Path::new(&image_fs).to_path_buf();
    let image_fs = if image_fs.is_relative() {
        std::env::current_dir().unwrap().join(image_fs)
    } else {
        image_fs
    };

    // 重要：image_fs的父目录挂载类型必须为private
    // 如果父目录不是挂载目录，执行sudo mount --bind --make-private $(parent) $(parent)

    let new_root = CString::new(image_fs.to_str().unwrap()).unwrap();
    let old_root = CString::new(image_fs.join(".old_root").to_str().unwrap()).unwrap();

    unsafe {
        let r = libc::unshare(libc::CLONE_NEWPID);
        assert_eq!(r, 0);

        let pid = libc::fork();
        assert!(pid >= 0);

        if pid == 0 {
            // child process
            let r = libc::mount(
                new_root.as_ptr(),
                new_root.as_ptr(),
                CString::new("").unwrap().as_ptr(),
                libc::MS_BIND | libc::MS_REC | libc::MS_PRIVATE,
                CString::new("").unwrap().as_ptr() as *const _,
            );
            assert_eq!(r, 0);

            let r = libc::mkdir(old_root.as_ptr(), 0755);
            assert_eq!(r, 0);

            // unshare新命名空间
            let r = libc::unshare(libc::CLONE_NEWNS | libc::CLONE_NEWNET);
            assert_eq!(r, 0);

            // 切换根文件系统
            let r = ffi::pivot_root(new_root.as_ptr(), old_root.as_ptr());
            assert_eq!(r, 0);

            let root_dir = CString::new("/").unwrap();
            libc::chdir(root_dir.as_ptr());

            // 挂载/proc
            let r = libc::mount(
                CString::new("").unwrap().as_ptr(),
                CString::new("/proc").unwrap().as_ptr(),
                CString::new("proc").unwrap().as_ptr(),
                libc::MS_NOEXEC | libc::MS_NOSUID | libc::MS_NODEV,
                CString::new("").unwrap().as_ptr() as *const _,
            );
            assert_eq!(r, 0);

            // 挂载/dev
            let r = libc::mount(
                CString::new("").unwrap().as_ptr(),
                CString::new("/dev").unwrap().as_ptr(),
                CString::new("devtmpfs").unwrap().as_ptr(),
                libc::MS_STRICTATIME | libc::MS_NOSUID,
                CString::new("mode=755").unwrap().as_ptr() as *const _,
            );
            assert_eq!(r, 0);

            // 防止umount事件扩散到宿主环境
            let old_root_2 = CString::new("/.old_root").unwrap();
            let r = libc::mount(
                CString::new("").unwrap().as_ptr(),
                old_root_2.as_ptr(),
                CString::new("").unwrap().as_ptr(),
                libc::MS_SLAVE | libc::MS_REC,
                CString::new("").unwrap().as_ptr() as *const _,
            );
            assert_eq!(r, 0);

            // umount /.old_root
            let r = libc::umount2(old_root_2.as_ptr(), libc::MNT_DETACH);
            assert_eq!(r, 0);

            libc::rmdir(old_root_2.as_ptr());

            let prog = CString::new(command).unwrap();
            let _ = libc::execve(
                prog.as_ptr(),
                command_args.as_ptr(),
                ptr::null::<libc::c_char>() as *const _,
            );
            unreachable!();
        } else {
            // parent process
            let _ = libc::wait4(
                pid,
                ptr::null::<i32>() as *mut _,
                0,
                ptr::null::<i32>() as *mut _,
            );

            libc::rmdir(old_root.as_ptr());
            libc::umount(new_root.as_ptr());
        }
    }
}

mod ffi {
    extern "C" {
        pub fn pivot_root(
            new_root: *const libc::c_char,
            put_old: *const libc::c_char,
        ) -> libc::c_int;
    }
}
