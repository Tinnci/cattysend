use anyhow::Result;
use clap::{Parser, Subcommand};
use xshell::{Shell, cmd};

#[derive(Parser)]
#[command(name = "xtask", about = "Cattysend 开发任务自动化")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 构建所有组件 (release)
    Build,
    /// 运行守护进程 (开发模式)
    Dev,
    /// 运行 TUI (开发模式)
    Tui {
        /// 日志级别 (trace, debug, info, warn, error)
        #[arg(short, long, default_value = "info")]
        log_level: String,
        /// 日志输出文件 (默认 /tmp/cattysend.log)
        #[arg(short = 'o', long)]
        log_file: Option<String>,
    },
    /// 安装 systemd 服务
    Install,
    /// 卸载 systemd 服务
    Uninstall,
    /// 设置 capabilities (免 sudo 运行)
    SetupCaps,
    /// 打包发布 (tar.gz)
    Dist,
    /// 运行测试
    Test,
    /// 运行测试并生成覆盖率报告
    Coverage,
    /// 清理构建产物
    Clean,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let sh = Shell::new()?;

    // 确保在项目根目录执行
    let project_root = std::env::var("CARGO_MANIFEST_DIR")
        .map(std::path::PathBuf::from)
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::env::current_dir().unwrap());
    sh.change_dir(&project_root);

    match cli.command {
        Commands::Build => build(&sh)?,
        Commands::Dev => dev(&sh)?,
        Commands::Tui {
            log_level,
            log_file,
        } => tui(&sh, &log_level, log_file)?,
        Commands::Install => install(&sh)?,
        Commands::Uninstall => uninstall(&sh)?,
        Commands::SetupCaps => setup_caps(&sh)?,
        Commands::Dist => dist(&sh)?,
        Commands::Test => test(&sh)?,
        Commands::Coverage => coverage(&sh)?,
        Commands::Clean => clean(&sh)?,
    }

    Ok(())
}

fn build(sh: &Shell) -> Result<()> {
    println!("🔨 构建所有组件...");
    cmd!(
        sh,
        "cargo build --release -p cattysend-daemon -p cattysend-cli -p cattysend-tui"
    )
    .run()?;
    println!("✅ 构建完成");
    Ok(())
}

fn dev(sh: &Shell) -> Result<()> {
    println!("🚀 启动开发模式守护进程...");
    cmd!(sh, "cargo run -p cattysend-daemon").run()?;
    Ok(())
}

fn tui(sh: &Shell, log_level: &str, log_file: Option<String>) -> Result<()> {
    let log_file = log_file.unwrap_or_else(|| "/tmp/cattysend.log".to_string());

    println!("🖥️  启动 TUI 调试模式...");
    println!("   日志级别: {}", log_level);
    println!("   日志文件: {}", log_file);
    println!("");
    println!("💡 提示: 在另一个终端运行以下命令查看实时日志:");
    println!("   tail -f {}", log_file);
    println!("");

    // 设置环境变量并运行
    let rust_log = format!(
        "{level},cattysend_core={level},bluer={level},btleplug=info",
        level = log_level
    );

    // 使用 shell 执行以支持重定向
    let command = format!(
        "RUST_LOG='{}' cargo run -p cattysend-tui 2>> '{}'",
        rust_log, log_file
    );

    cmd!(sh, "bash -c {command}").run()?;

    println!("");
    println!("📁 日志已保存到: {}", log_file);
    Ok(())
}

fn install(sh: &Shell) -> Result<()> {
    println!("📦 安装 Cattysend 服务...");

    // 构建
    build(sh)?;

    // 先停止已运行的服务（如果存在）
    println!("⏹️  停止现有服务...");
    let _ = cmd!(sh, "sudo systemctl stop cattysend.service").run();

    // 等待进程完全退出
    std::thread::sleep(std::time::Duration::from_millis(500));

    // 复制二进制文件
    println!("📋 复制二进制文件...");
    cmd!(
        sh,
        "sudo cp target/release/cattysend-daemon /usr/local/bin/"
    )
    .run()?;
    cmd!(
        sh,
        "sudo cp target/release/cattysend-cli /usr/local/bin/cattysend"
    )
    .run()?;

    // 复制 TUI（如果存在）
    if std::path::Path::new("target/release/cattysend-tui").exists() {
        cmd!(sh, "sudo cp target/release/cattysend-tui /usr/local/bin/").run()?;
    }

    // 复制 systemd 服务文件
    cmd!(sh, "sudo cp assets/cattysend.service /etc/systemd/system/").run()?;

    // 启用并启动服务
    cmd!(sh, "sudo systemctl daemon-reload").run()?;
    cmd!(sh, "sudo systemctl enable --now cattysend.service").run()?;

    println!("✅ 服务安装完成");
    println!("   使用 'cattysend --help' 查看命令");
    println!("   使用 'cattysend-tui' 启动交互界面");
    println!("   使用 'systemctl status cattysend' 查看服务状态");
    Ok(())
}

fn uninstall(sh: &Shell) -> Result<()> {
    println!("🗑️  卸载 Cattysend 服务...");

    // 停止并禁用服务
    let _ = cmd!(sh, "sudo systemctl stop cattysend.service").run();
    let _ = cmd!(sh, "sudo systemctl disable cattysend.service").run();

    // 删除文件
    let _ = cmd!(sh, "sudo rm /etc/systemd/system/cattysend.service").run();
    let _ = cmd!(sh, "sudo rm /usr/local/bin/cattysend-daemon").run();
    let _ = cmd!(sh, "sudo rm /usr/local/bin/cattysend").run();

    cmd!(sh, "sudo systemctl daemon-reload").run()?;

    println!("✅ 卸载完成");
    Ok(())
}

fn setup_caps(sh: &Shell) -> Result<()> {
    println!("🔐 设置 capabilities (免 sudo 网络操作)...");

    build(sh)?;

    // CAP_NET_ADMIN: WiFi P2P 操作
    // CAP_NET_RAW: BLE 扫描
    cmd!(
        sh,
        "sudo setcap 'cap_net_admin,cap_net_raw+eip' target/release/cattysend-daemon"
    )
    .run()?;

    println!("✅ Capabilities 设置完成");
    println!("   守护进程现在可以免 sudo 运行");
    Ok(())
}

fn dist(sh: &Shell) -> Result<()> {
    println!("📦 打包发布...");

    build(sh)?;

    let version = "0.1.0";
    let dist_name = format!("cattysend-{}-linux-x86_64", version);

    cmd!(sh, "mkdir -p dist/{dist_name}").run()?;
    cmd!(sh, "cp target/release/cattysend-daemon dist/{dist_name}/").run()?;
    cmd!(
        sh,
        "cp target/release/cattysend-cli dist/{dist_name}/cattysend"
    )
    .run()?;
    cmd!(sh, "cp assets/cattysend.service dist/{dist_name}/").run()?;
    cmd!(sh, "cp README.md dist/{dist_name}/ || true").run()?;

    sh.change_dir("dist");
    cmd!(sh, "tar -czvf {dist_name}.tar.gz {dist_name}").run()?;

    println!("✅ 打包完成: dist/{}.tar.gz", dist_name);
    Ok(())
}

fn test(sh: &Shell) -> Result<()> {
    println!("🧪 运行测试...");
    cmd!(sh, "cargo test --workspace").run()?;
    println!("✅ 测试完成");
    Ok(())
}

fn coverage(sh: &Shell) -> Result<()> {
    println!("📊 运行测试覆盖率分析...");

    // 检查 cargo-tarpaulin 是否安装
    if cmd!(sh, "cargo tarpaulin --version").run().is_err() {
        println!("📦 安装 cargo-tarpaulin...");
        cmd!(sh, "cargo install cargo-tarpaulin").run()?;
    }

    // 运行覆盖率分析
    println!("🔍 分析中...");
    cmd!(
        sh,
        "cargo tarpaulin --packages cattysend-core --out Html --output-dir target/coverage"
    )
    .run()?;

    println!("✅ 覆盖率报告已生成");
    println!("   HTML 报告: target/coverage/tarpaulin-report.html");
    Ok(())
}

fn clean(sh: &Shell) -> Result<()> {
    println!("🧹 清理构建产物...");
    cmd!(sh, "cargo clean").run()?;
    cmd!(sh, "rm -rf dist").run()?;
    println!("✅ 清理完成");
    Ok(())
}
