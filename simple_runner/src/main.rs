use std::{
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Command, Stdio},
};

use bootloader::BootConfig;
use ovmf_prebuilt::{Arch, FileType, Prebuilt, Source};

fn main() {
    let mut args = std::env::args();

    let _function = args.next();

    let kernel = PathBuf::from(args.next().expect("Expected a path to be passed"));

    let mut path = kernel.clone();
    path.pop();

    // Create the UEFI disk image.
    let uefi_path = path.join("uefi.img");

    let mut bootconfig = BootConfig::default();

    bootconfig.serial_logging = false;

    bootloader::UefiBoot::new(&kernel)
        .set_boot_config(&bootconfig)
        .create_disk_image(&uefi_path)
        .expect("failed to create UEFI disk image");

    let prebuilt =
        Prebuilt::fetch(Source::LATEST, "target/ovmf").expect("failed to update prebuilt");

    let code = prebuilt.get_file(Arch::X64, FileType::Code);
    let vars = prebuilt.get_file(Arch::X64, FileType::Vars);

    let mut cmd = Command::new("qemu-system-x86_64");

    // Forward the guest's serial output directly to Cargo's terminal.
    cmd.arg("-serial").arg("stdio");

    // Don't open a graphical QEMU window.
    // cmd.arg("-display").arg("hidden");

    // Number of virtual CPUs.
    cmd.arg("-smp").arg("4");

    // Allow the guest to terminate QEMU.
    cmd.arg("-device")
        .arg("isa-debug-exit,iobase=0xf4,iosize=0x04");

    // UEFI disk image.
    cmd.arg("-drive")
        .arg(format!("format=raw,file={}", uefi_path.display()));

    // UEFI firmware.
    cmd.arg("-drive").arg(format!(
        "if=pflash,format=raw,unit=0,file={},readonly=on",
        code.display()
    ));

    // UEFI variables.
    cmd.arg("-drive").arg(format!(
        "if=pflash,format=raw,unit=1,file={},snapshot=on",
        vars.display()
    ));

    // Make QEMU's stdout/stderr appear as the runner's stdout/stderr.
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start qemu-system-x86_64");

    let mut kernal_started = false;
    for line in BufReader::new((child.stdout.take()).unwrap()).lines() {
        if let Ok(line) = line {
            if kernal_started {
                println!("{}", line);
            }

            if !kernal_started && line.contains("CuteKernelInit") {
                kernal_started = true;
            }
        } else {
            break;
        }
    }

    let status = child.wait().expect("failed to wait on qemu");

    let exit_code = match status.code().unwrap_or(1) {
        0x21 => 0, // test success
        0x11 => 1, // test failure
        code => {
            eprintln!("QEMU exited with unexpected code: {code:#x}");
            2
        }
    };

    std::process::exit(exit_code);
}
