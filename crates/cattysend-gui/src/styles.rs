//! CSS 样式定义
//!
//! 现代化的深色主题 GUI 样式

//! 主题颜色
//!
//! 这些常量保留用于未来的动态主题切换功能。
//! 当前 GLOBAL_CSS 使用硬编码值，但这些常量为未来的 Rust 端主题逻辑
//! 提供了一致的颜色定义参考。
#[expect(dead_code, reason = "reserved for future dynamic theming")]
pub mod colors {
    pub const PRIMARY: &str = "#FACC15"; // Yellow
    pub const SECONDARY: &str = "#F472B6"; // Pink
    pub const ACCENT: &str = "#22D3EE"; // Cyan
    pub const SUCCESS: &str = "#4ADE80"; // Green
    pub const ERROR: &str = "#FB7185"; // Rose
    pub const BG: &str = "#F8FAFC"; // Slate 50
    pub const DARK_BG: &str = "#0F172A"; // Slate 900
    pub const BORDER: &str = "#000000"; // Black
    pub const TEXT: &str = "#000000";
}

/// 全局 CSS 样式
pub const GLOBAL_CSS: &str = r#"
* {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
}

:root {
    --primary: #FACC15;
    --secondary: #F472B6;
    --accent: #22D3EE;
    --success: #4ADE80;
    --error: #FB7185;
    --bg: #F8FAFC;
    --border: #000000;
    --shadow: 6px 6px 0px #000000;
    --shadow-sm: 4px 4px 0px #000000;
    --font-main: 'Outfit', 'Inter', sans-serif;
}

body {
    font-family: var(--font-main);
    background-color: var(--bg);
    color: var(--border);
    padding: 24px;
    line-height: 1.5;
}

/* Bento Grid Layout */
.app-container {
    max-width: 1200px;
    margin: 0 auto;
    display: grid;
    grid-template-columns: repeat(12, 1fr);
    grid-auto-rows: minmax(100px, auto);
    gap: 20px;
    min-width: 800px; /* Enforce minimum width as requested */
}

/* Boxes (Bento Tiles) */
.bento-tile {
    background: white;
    border: 3px solid var(--border);
    box-shadow: var(--shadow);
    padding: 24px;
    transition: all 0.2s cubic-bezier(0.175, 0.885, 0.32, 1.275);
    position: relative;
    overflow: hidden;
}

.bento-tile:hover {
    transform: translate(-2px, -2px);
    box-shadow: 8px 8px 0px var(--border);
}

/* Specific Layout Roles */
.header-tile {
    grid-column: span 12;
    background: var(--primary);
    display: flex;
    justify-content: space-between;
    align-items: center;
}

.mode-tile {
    grid-column: span 12;
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 20px;
    padding: 0;
    background: transparent;
    border: none;
    box-shadow: none;
}

.mode-tile:hover {
    transform: none;
    box-shadow: none;
}

.main-left {
    grid-column: span 7;
    background: white;
}

.main-right {
    grid-column: span 5;
    background: var(--accent);
}

/* Typography */
h1 { font-size: 32px; font-weight: 900; letter-spacing: -1px; }
h2 { font-size: 24px; font-weight: 800; margin-bottom: 16px; }

/* Buttons */
.btn {
    font-family: inherit;
    font-weight: 800;
    padding: 12px 24px;
    border: 3px solid var(--border);
    box-shadow: var(--shadow-sm);
    cursor: pointer;
    background: white;
    transition: all 0.1s;
    text-transform: uppercase;
    display: inline-flex;
    align-items: center;
    gap: 10px;
}

.btn:active {
    transform: translate(2px, 2px);
    box-shadow: 0px 0px 0px var(--border);
}

.btn-primary { background: var(--primary); }
.btn-secondary { background: var(--secondary); }
.btn-accent { background: var(--accent); }

/* Status Badge */
.status-badge {
    background: white;
    border: 2px solid var(--border);
    padding: 6px 12px;
    font-weight: 700;
    box-shadow: 2px 2px 0px var(--border);
}

.status-badge.scanning { background: var(--primary); }
.status-badge.error { background: var(--error); color: white; }

/* Device List & Items */
.device-list {
    display: flex;
    flex-direction: column;
    gap: 16px;
}

.device-item {
    border: 3px solid var(--border);
    padding: 16px;
    background: white;
    display: flex;
    align-items: center;
    gap: 16px;
    cursor: pointer;
    transition: all 0.2s;
    box-shadow: 4px 4px 0px var(--border);
}

.device-item:hover {
    transform: translate(-2px, -2px);
    box-shadow: 6px 6px 0px var(--border);
    background: #FFFBEB; /* Light yellowish */
}

.device-item.selected {
    background: var(--primary);
}

.device-icon {
    width: 50px;
    height: 50px;
    background: white;
    border: 2px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 24px;
    box-shadow: 2px 2px 0px var(--border);
}

/* Progress */
.progress-container {
    border: 3px solid var(--border);
    height: 32px;
    background: white;
    box-shadow: 4px 4px 0px var(--border);
    position: relative;
    overflow: hidden;
}

.progress-fill {
    height: 100%;
    background: var(--secondary);
    border-right: 3px solid var(--border);
    transition: width 0.3s ease;
}

.progress-text {
    position: absolute;
    width: 100%;
    text-align: center;
    top: 50%;
    transform: translateY(-50%);
    font-weight: 800;
    color: black;
    text-shadow: 1px 1px 0px white;
}

/* Dropzone */
.dropzone {
    border: 4px dashed var(--border);
    background: transparent;
    padding: 40px;
    text-align: center;
    transition: all 0.2s;
}

.dropzone:hover {
    background: white;
    border-style: solid;
}

/* Receive Mode Specifics */
.receive-log {
    background: #000;
    color: #4ADE80;
    padding: 20px;
    border: 3px solid var(--border);
    font-family: 'Courier New', monospace;
    font-size: 14px;
    box-shadow: var(--shadow-sm);
    flex: 1;
    overflow-y: auto;
}

/* Mode Selection Cards */
.mode-card {
    background: white;
    border: 3px solid var(--border);
    box-shadow: var(--shadow-sm);
    padding: 24px;
    text-align: center;
    cursor: pointer;
    transition: all 0.2s;
}

.mode-card:hover {
    transform: translate(-4px, -4px);
    box-shadow: 8px 8px 0px var(--border);
}

.mode-card.active {
    background: var(--primary);
}

.mode-card-icon {
    font-size: 40px;
    margin-bottom: 12px;
}

.mode-card-title {
    font-weight: 900;
    text-transform: uppercase;
}

/* === Adaptive Layout & Responsiveness === */
@media (max-width: 900px) {
    .app-container {
        grid-template-columns: repeat(6, 1fr);
    }
    /* Main layout adjustment for smaller screens */
    .main-left, .main-right {
        grid-column: span 6 !important;
    }
}

@media (max-width: 600px) {
    .app-container {
        display: flex;
        flex-direction: column;
    }
    .mode-tile {
        grid-template-columns: 1fr;
    }
    .btn {
        width: 100%;
        justify-content: center;
    }
}

/* === Refined Animations === */
@keyframes pulse-ring {
    0% { transform: scale(0.8); opacity: 0.8; border-width: 6px; }
    50% { opacity: 0.5; }
    100% { transform: scale(2.2); opacity: 0; border-width: 0px; }
}

@keyframes bounce-subtle {
    0%, 100% { transform: translateY(0); }
    50% { transform: translateY(-6px); }
}

@keyframes spin-ease {
    0% { transform: rotate(0deg); }
    100% { transform: rotate(360deg); }
}

@keyframes fade-in-up {
    from { opacity: 0; transform: translateY(20px); }
    to { opacity: 1; transform: translateY(0); }
}

/* === Optimized Receiver UI === */
.receive-wrapper {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 500px; /* Base height assurance */
}

.receive-container {
    flex: 1; /* Fill available space */
    width: 100%;
    padding: 32px;
    background: radial-gradient(circle at center, #ffffff 0%, #f8fafc 100%);
    border: 3px solid var(--border);
    margin-bottom: 20px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    position: relative;
    overflow: hidden;
    box-shadow: inset 0 0 20px rgba(0,0,0,0.02);
    /* Animation entry */
    animation: fade-in-up 0.4s ease-out;
}

/* Radar / Broadcasting Effect - High Fidelity */
.radar-box {
    position: relative;
    width: 200px;
    height: 200px;
    display: flex;
    justify-content: center;
    align-items: center;
    margin-bottom: 32px;
}

.radar-emitter {
    width: 64px;
    height: 64px;
    background: var(--accent);
    border: 3px solid var(--border);
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 32px;
    z-index: 20;
    position: relative;
    box-shadow: 4px 4px 0px var(--border);
    transition: transform 0.2s;
}

.radar-emitter:hover {
    transform: scale(1.1);
}

.radar-ring {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: 64px;
    height: 64px;
    border: 2px solid var(--accent);
    border-radius: 50%;
    opacity: 0;
    box-sizing: border-box;
    pointer-events: none;
    z-index: 5; /* Ensure rings are below the emitter and text pills */
    background: transparent; /* Ensure no accidental background covers content */
}

.radar-ring.animating {
    animation: pulse-ring 2.5s cubic-bezier(0.25, 0.46, 0.45, 0.94) infinite;
}

.radar-ring:nth-child(1) { animation-delay: 0s; }
.radar-ring:nth-child(2) { animation-delay: 0.8s; }
.radar-ring:nth-child(3) { animation-delay: 1.6s; }

/* Status Text Badge - Modernized */
.status-pill {
    background: white;
    border: 2px solid var(--border);
    padding: 10px 24px;
    font-weight: 800;
    font-size: 16px;
    display: inline-flex;
    align-items: center;
    gap: 10px;
    box-shadow: 4px 4px 0px rgba(0,0,0,0.1);
    border-radius: 40px;
    z-index: 30; /* Ensure text is always above effects */
    transition: all 0.3s;
    background: white; /* Ensure background is solid */
    position: relative;
}

.status-pill:hover {
    transform: translateY(-2px);
    box-shadow: 4px 6px 0px rgba(0,0,0,0.15);
}

.status-pill.error {
    border-color: var(--error);
    color: var(--error);
    background: #fffafa;
}

/* File Transfer Card - Adaptive */
.rx-file-card {
    width: 100%;
    max-width: 500px;
    border: 3px solid var(--border);
    background: white;
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 20px;
    box-shadow: 8px 8px 0px var(--border);
    z-index: 20;
    position: relative;
    /* Card Animation */
    animation: fade-in-up 0.5s cubic-bezier(0.175, 0.885, 0.32, 1.275);
}

.rx-file-header {
    display: flex;
    align-items: center;
    gap: 16px;
}

.rx-file-icon {
    font-size: 40px;
    animation: bounce-subtle 2s infinite ease-in-out;
}

.rx-file-details {
    flex: 1;
    overflow: hidden;
}

.rx-file-name {
    font-weight: 800;
    font-size: 18px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    margin-bottom: 4px;
}

.rx-file-status {
    font-size: 14px;
    color: #64748B;
    font-weight: 500;
}

/* Connecting Spinner - Smoother */
.spinner {
    width: 48px;
    height: 48px;
    border: 5px solid #e2e8f0;
    border-top: 5px solid var(--primary);
    border-radius: 50%;
    animation: spin-ease 1s cubic-bezier(0.55, 0.055, 0.675, 0.19) infinite;
    margin-bottom: 24px;
}
"#;
