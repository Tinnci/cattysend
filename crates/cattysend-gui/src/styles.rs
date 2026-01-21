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
"#;
