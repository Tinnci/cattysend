//! CSS 样式定义
//!
//! 现代化的深色主题 GUI 样式

//! 主题颜色
#[allow(dead_code)]
pub mod colors {
    pub const PRIMARY: &str = "#6366f1"; // Indigo
    pub const PRIMARY_HOVER: &str = "#4f46e5";
    pub const SECONDARY: &str = "#8b5cf6"; // Violet
    pub const SUCCESS: &str = "#22c55e"; // Green
    pub const WARNING: &str = "#f59e0b"; // Amber
    pub const ERROR: &str = "#ef4444"; // Red
    pub const INFO: &str = "#3b82f6"; // Blue

    pub const BG_DARK: &str = "#0f172a"; // Slate 900
    pub const BG_CARD: &str = "#1e293b"; // Slate 800
    pub const BG_HOVER: &str = "#334155"; // Slate 700
    pub const BORDER: &str = "#475569"; // Slate 600

    pub const TEXT_PRIMARY: &str = "#f1f5f9"; // Slate 100
    pub const TEXT_SECONDARY: &str = "#94a3b8"; // Slate 400
    pub const TEXT_MUTED: &str = "#64748b"; // Slate 500
}

/// 全局 CSS 样式
pub const GLOBAL_CSS: &str = r#"
* {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
}

body {
    font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    background: linear-gradient(135deg, #0f172a 0%, #1e1b4b 100%);
    color: #f1f5f9;
    min-height: 100vh;
}

/* 容器 */
.app-container {
    display: flex;
    flex-direction: column;
    min-height: 100vh;
    max-width: 1200px;
    margin: 0 auto;
    padding: 20px;
}

/* 头部 */
.header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px 24px;
    background: rgba(30, 41, 59, 0.8);
    backdrop-filter: blur(10px);
    border-radius: 16px;
    margin-bottom: 20px;
    border: 1px solid rgba(71, 85, 105, 0.5);
}

.logo {
    display: flex;
    align-items: center;
    gap: 12px;
}

.logo h1 {
    font-size: 24px;
    font-weight: 700;
    background: linear-gradient(135deg, #6366f1, #8b5cf6);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
}

.status-badge {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 16px;
    background: rgba(34, 197, 94, 0.2);
    border-radius: 20px;
    font-size: 14px;
    color: #22c55e;
}

.status-badge.scanning {
    background: rgba(59, 130, 246, 0.2);
    color: #3b82f6;
}

.status-badge.error {
    background: rgba(239, 68, 68, 0.2);
    color: #ef4444;
}

/* 主内容区 */
.main-content {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 20px;
    flex: 1;
}

/* 卡片 */
.card {
    background: rgba(30, 41, 59, 0.8);
    backdrop-filter: blur(10px);
    border-radius: 16px;
    padding: 24px;
    border: 1px solid rgba(71, 85, 105, 0.5);
}

.card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 20px;
}

.card-title {
    font-size: 18px;
    font-weight: 600;
    color: #f1f5f9;
}

/* 按钮 */
.btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 12px 24px;
    border: none;
    border-radius: 12px;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s ease;
}

.btn-primary {
    background: linear-gradient(135deg, #6366f1, #8b5cf6);
    color: white;
}

.btn-primary:hover {
    transform: translateY(-2px);
    box-shadow: 0 10px 20px rgba(99, 102, 241, 0.3);
}

.btn-secondary {
    background: rgba(51, 65, 85, 0.8);
    color: #f1f5f9;
    border: 1px solid rgba(71, 85, 105, 0.5);
}

.btn-secondary:hover {
    background: rgba(71, 85, 105, 0.8);
}

.btn-icon {
    padding: 10px;
    border-radius: 10px;
}

.btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
    transform: none;
}

/* 设备列表 */
.device-list {
    display: flex;
    flex-direction: column;
    gap: 12px;
    max-height: 400px;
    overflow-y: auto;
}

.device-item {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 16px;
    background: rgba(51, 65, 85, 0.5);
    border-radius: 12px;
    cursor: pointer;
    transition: all 0.2s ease;
    border: 1px solid transparent;
}

.device-item:hover {
    background: rgba(71, 85, 105, 0.5);
    border-color: rgba(99, 102, 241, 0.5);
}

.device-item.selected {
    border-color: #6366f1;
    background: rgba(99, 102, 241, 0.2);
}

.device-icon {
    width: 48px;
    height: 48px;
    border-radius: 12px;
    background: linear-gradient(135deg, #6366f1, #8b5cf6);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 24px;
}

.device-info {
    flex: 1;
}

.device-name {
    font-size: 16px;
    font-weight: 600;
    color: #f1f5f9;
}

.device-address {
    font-size: 12px;
    color: #94a3b8;
    margin-top: 4px;
}

.device-rssi {
    font-size: 12px;
    color: #64748b;
    display: flex;
    align-items: center;
    gap: 4px;
}

/* 进度条 */
.progress-container {
    margin: 20px 0;
}

.progress-bar {
    height: 8px;
    background: rgba(51, 65, 85, 0.8);
    border-radius: 4px;
    overflow: hidden;
}

.progress-fill {
    height: 100%;
    background: linear-gradient(90deg, #6366f1, #8b5cf6);
    border-radius: 4px;
    transition: width 0.3s ease;
}

.progress-text {
    display: flex;
    justify-content: space-between;
    margin-top: 8px;
    font-size: 12px;
    color: #94a3b8;
}

/* 文件拖放区 */
.dropzone {
    border: 2px dashed rgba(99, 102, 241, 0.5);
    border-radius: 16px;
    padding: 40px;
    text-align: center;
    transition: all 0.2s ease;
    cursor: pointer;
}

.dropzone:hover,
.dropzone.active {
    border-color: #6366f1;
    background: rgba(99, 102, 241, 0.1);
}

.dropzone-icon {
    font-size: 48px;
    margin-bottom: 16px;
}

.dropzone-text {
    font-size: 16px;
    color: #94a3b8;
}

.dropzone-hint {
    font-size: 12px;
    color: #64748b;
    margin-top: 8px;
}

/* 模式选择 */
.mode-selector {
    display: flex;
    gap: 12px;
    margin-bottom: 20px;
}

.mode-btn {
    flex: 1;
    padding: 20px;
    background: rgba(30, 41, 59, 0.8);
    border: 1px solid rgba(71, 85, 105, 0.5);
    border-radius: 16px;
    cursor: pointer;
    transition: all 0.2s ease;
    text-align: center;
}

.mode-btn:hover {
    border-color: rgba(99, 102, 241, 0.5);
    transform: translateY(-2px);
}

.mode-btn.active {
    border-color: #6366f1;
    background: rgba(99, 102, 241, 0.2);
}

.mode-btn-icon {
    font-size: 32px;
    margin-bottom: 12px;
}

.mode-btn-title {
    font-size: 16px;
    font-weight: 600;
    color: #f1f5f9;
}

.mode-btn-desc {
    font-size: 12px;
    color: #94a3b8;
    margin-top: 4px;
}

/* 动画 */
@keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.5; }
}

.scanning .status-dot {
    animation: pulse 1.5s ease-in-out infinite;
}

.status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: currentColor;
}

/* 滚动条 */
::-webkit-scrollbar {
    width: 6px;
}

::-webkit-scrollbar-track {
    background: transparent;
}

::-webkit-scrollbar-thumb {
    background: rgba(71, 85, 105, 0.5);
    border-radius: 3px;
}

::-webkit-scrollbar-thumb:hover {
    background: rgba(71, 85, 105, 0.8);
}

/* 空状态 */
.empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 40px;
    text-align: center;
    color: #64748b;
}

.empty-state-icon {
    font-size: 48px;
    margin-bottom: 16px;
    opacity: 0.5;
}

.empty-state-text {
    font-size: 14px;
}
"#;
