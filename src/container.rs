use std::ffi::CString;
use std::fs::{self, File};
use std::io::prelude::*;
use std::iter;
use std::path::Path;

use nix::sched::{unshare, CloneFlags};
use nix::sys::wait::{waitpid, WaitStatus};
use nix::unistd::{chdir, chroot, fork, getgid, getpid, getuid, ForkResult, Gid, Uid};
use nix::unistd::{execve, sethostname};

use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

use log::info;

use super::image::Image;
use super::mounts;

pub struct Container {
    pub id: String,
    pub name: String,
    pub command: String,
    pub image: Image,
    pub host_uid: Uid,
    pub host_gid: Gid,
    pub path: String, // for --path option
}

impl Container {
    pub fn new(name: &str, command: String, path: Option<&str>) -> Container {
        let mut rng = thread_rng();

        if let Some(path) = path {
            return Container {
                id: Path::new(path)
                    .file_stem()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string(),
                name: name.to_string(),
                command,
                image: Image::new(name),
                host_uid: getuid(),
                host_gid: getgid(),
                path: path.to_string(),
            };
        }

        let id: String = iter::repeat(())
            .map(|()| rng.sample(Alphanumeric))
            .take(16)
            .collect();

        Container {
            id,
            name: name.to_string(),
            command,
            image: Image::new(name),
            host_uid: getuid(),
            host_gid: getgid(),
            path: "".to_string(),
        }
    }

    fn uid_map(&self) -> std::io::Result<()> {
        let mut uid_map_file = File::create("/proc/self/uid_map")?;
        let uid_map = format!("0 {} 1", self.host_uid);

        uid_map_file.write_all(uid_map.as_bytes())?;
        info!("[Host] wrote {} /proc/self/uid_map", uid_map);
        Ok(())
    }

    fn gid_map(&self) -> std::io::Result<()> {
        let mut setgroups_file = File::create("/proc/self/setgroups")?;
        setgroups_file.write_all(b"deny")?;

        let mut gid_map_file = File::create("/proc/self/gid_map")?;
        info!("[Host] open(2) /proc/self/gid_map done.");
        let gid_map = format!("0 {} 1", self.host_gid);

        gid_map_file.write_all(gid_map.as_bytes())?;
        info!("[Host] wrote {} /proc/self/gid_map", gid_map);
        Ok(())
    }

    fn guid_map(&self) -> std::io::Result<()> {
        self.uid_map().expect("Failed to write uid_map");
        self.gid_map().expect("Failed to write gid_map");
        Ok(())
    }

    pub fn prepare(&mut self) {
        if self.path == "" {
            self.image.pull(&self.id).expect("Failed to cromwell pull");

            let c_hosts = format!("{}/etc/hosts", self.image.get_full_path(&self.id));
            let c_resolv = format!("{}/etc/resolv.conf", self.image.get_full_path(&self.id));

            fs::copy("/etc/hosts", &c_hosts).expect("Failed copy /etc/hosts");
            info!("[Host] Copied /etc/hosts to {}", c_hosts);

            fs::copy("/etc/resolv.conf", &c_resolv).expect("Failed copy /etc/resolv.conf");
            info!("[Host] Copied /etc/resolv.conf {}", c_resolv);
        }

        unshare(
            CloneFlags::CLONE_NEWPID
                | CloneFlags::CLONE_NEWUTS
                | CloneFlags::CLONE_NEWNS
                | CloneFlags::CLONE_NEWUSER,
        )
        .expect("Can not unshare(2).");

        self.guid_map()
            .expect("Failed to write /proc/self/gid_map|uid_map");

        chroot(self.image.get_full_path(&self.id).as_str()).expect("chroot failed.");
        chdir("/").expect("cd / failed.");

        sethostname(&self.name).expect("Could not set hostname");
    }

    pub fn run(&self) {
        match fork() {
            Ok(ForkResult::Parent { child, .. }) => {
                info!("[Host] PID: {}", getpid());
                info!("[Container] PID: {}", child);

                match waitpid(child, None).expect("waitpid faild") {
                    WaitStatus::Exited(_, _) => {}
                    WaitStatus::Signaled(_, _, _) => {}
                    _ => eprintln!("Unexpected exit."),
                }
            }
            Ok(ForkResult::Child) => {
                fs::create_dir_all("proc").unwrap_or_else(|why| {
                    eprintln!("{:?}", why.kind());
                });

                info!("[Container] Mount procfs ... ");
                mounts::mount_proc().expect("mount procfs failed");

                let cmd = CString::new(self.command.clone()).unwrap();
                let default_shell = CString::new("/bin/sh").unwrap();
                let shell_opt = CString::new("-c").unwrap();
                let lang = CString::new("LC_ALL=C").unwrap();
                let path =
                    CString::new("PATH=/bin/:/usr/bin/:/usr/local/bin:/sbin:/usr/sbin").unwrap();

                execve(
                    &default_shell,
                    &[default_shell.clone(), shell_opt, cmd],
                    &[lang, path],
                )
                .expect("execution faild.");
            }
            Err(e) => panic!("Fork failed: {}", e),
        }
    }

    pub fn delete(&self) -> std::io::Result<()> {
        fs::remove_dir_all(&self.image.get_full_path(&self.id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_container() {
        let image_name = "library/alpine:3.8";
        let command = "/bin/bash".to_string();
        let container = Container::new(image_name, command.clone());
        assert_eq!(container.command, command);
    }
}
