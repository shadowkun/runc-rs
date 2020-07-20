use crate::Result;
use rand::distributions::Alphanumeric;
use rand::Rng;
use std::process::Command;

const BRIDGE_NAME: &str = "runc-rs";

// 启动设备
fn up_dev(dev: &str) -> Result<()> {
    let output = Command::new("ip")
        .args(&["link", "set", "dev", dev, "up"])
        .output()?;
    if !output.status.success() {
        let msg = String::from_utf8_lossy(output.stderr.as_slice()).to_string();
        panic!(msg);
    }
    Ok(())
}

// 启动设备
fn up_dev_ns(dev: &str, ns_id: &str) -> Result<()> {
    let output = Command::new("ip")
        .args(&[
            "netns", "exec", ns_id, "ip", "link", "set", "dev", dev, "up",
        ])
        .output()?;
    if !output.status.success() {
        let msg = String::from_utf8_lossy(output.stderr.as_slice()).to_string();
        panic!(msg);
    }
    Ok(())
}

// 初始化网桥
pub fn init_bridge() -> Result<()> {
    let output = Command::new("ip")
        .args(&["-j", "link", "show", BRIDGE_NAME])
        .output()?;
    if output.status.success() {
        up_dev(BRIDGE_NAME)?;
        return Ok(());
    }
    // 创建网桥接口
    let output = Command::new("ip")
        .args(&["link", "add", BRIDGE_NAME, "type", "bridge"])
        .output()?;
    if !output.status.success() {
        let msg = String::from_utf8_lossy(output.stderr.as_slice()).to_string();
        panic!(msg);
    }
    up_dev(BRIDGE_NAME)?;
    Ok(())
}

// 创建虚拟设备对
pub fn create_veth() -> Result<(String, String)> {
    // 生成随机虚拟设备名
    let veth0 = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(6)
        .collect::<String>();
    let veth0 = format!("{}-{}", BRIDGE_NAME, veth0.to_lowercase());
    let veth1 = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(6)
        .collect::<String>();
    let veth1 = format!("{}-{}", BRIDGE_NAME, veth1.to_lowercase());

    let output = Command::new("ip")
        .args(&[
            "link",
            "add",
            veth0.as_str(),
            "type",
            "veth",
            "peer",
            "name",
            veth1.as_str(),
        ])
        .output()?;
    if !output.status.success() {
        let msg = String::from_utf8_lossy(output.stderr.as_slice()).to_string();
        panic!(msg);
    }
    Ok((veth0, veth1))
}

// 获取某个进程的网络命名空间
pub fn find_ns_net(pid: libc::pid_t) -> Result<String> {
    let output = Command::new("lsns")
        .args(&[
            "-t",
            "net",
            "-o",
            "NS",
            "-n",
            "-p",
            pid.to_string().as_str(),
        ])
        .output()?;
    if !output.status.success() {
        let msg = String::from_utf8_lossy(output.stderr.as_slice()).to_string();
        panic!(msg);
    }

    let ns_id = String::from_utf8_lossy(output.stdout.as_slice())
        .trim()
        .to_string();
    Ok(ns_id)
}

// 添加进netns
pub fn put_netns(pid: libc::pid_t, ns_id: &str) -> Result<()> {
    let output = Command::new("ln")
        .args(&[
            "-s",
            format!("/proc/{}/ns/net", pid).as_str(),
            format!("/var/run/netns/{}", ns_id).as_str(),
        ])
        .output()?;
    if !output.status.success() {
        let msg = String::from_utf8_lossy(output.stderr.as_slice()).to_string();
        panic!(msg);
    }
    Ok(())
}

// 释放netns
pub fn release_netns(ns_id: &str) -> Result<()> {
    let output = Command::new("rm")
        .args(&[format!("/var/run/netns/{}", ns_id).as_str()])
        .output()?;
    if !output.status.success() {
        let msg = String::from_utf8_lossy(output.stderr.as_slice()).to_string();
        panic!(msg);
    }
    Ok(())
}

// 将虚拟设备连接到网桥
pub fn link_veth_to_bridge(veth: &str) -> Result<()> {
    let output = Command::new("ip")
        .args(&["link", "set", "dev", veth, "master", BRIDGE_NAME])
        .output()?;
    if !output.status.success() {
        let msg = String::from_utf8_lossy(output.stderr.as_slice()).to_string();
        panic!(msg);
    }
    up_dev(veth)?;
    Ok(())
}

// 将虚拟设备放入命名空间
pub fn link_veth_to_ns(veth: &str, ns_id: &str, ip: &str) -> Result<()> {
    let output = Command::new("ip")
        .args(&["link", "set", "dev", veth, "netns", ns_id])
        .output()?;
    if !output.status.success() {
        let msg = String::from_utf8_lossy(output.stderr.as_slice()).to_string();
        panic!(msg);
    }
    // 重命名
    let output = Command::new("ip")
        .args(&[
            "netns", "exec", ns_id, "ip", "link", "set", "dev", veth, "name", "eth0",
        ])
        .output()?;
    if !output.status.success() {
        let msg = String::from_utf8_lossy(output.stderr.as_slice()).to_string();
        panic!(msg);
    }
    // 配置ip
    let output = Command::new("ip")
        .args(&[
            "netns", "exec", ns_id, "ip", "addr", "add", ip, "dev", "eth0",
        ])
        .output()?;
    if !output.status.success() {
        let msg = String::from_utf8_lossy(output.stderr.as_slice()).to_string();
        panic!(msg);
    }
    // 启动设备
    up_dev_ns("eth0", ns_id)?;
    up_dev_ns("lo", ns_id)?;
    Ok(())
}

