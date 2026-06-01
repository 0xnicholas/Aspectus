# Aspectus Console vs Logto Console — 管理面板对比

> Aspectus Console: v0.1（新建） | Logto Console: latest (2026)

---

## 1. 规模对比

| 维度 | Logto Console | Aspectus Console |
|------|:--:|:--:|
| 文件数 | **1,460** (.tsx/.ts/.scss) | **10** |
| 代码量 | **78,000 行** TypeScript | **~500 行** TypeScript |
| 页面数 | **40+** 路由页面 | **6** 页面 |
| 设计系统 | 自建 **50+** ds-components | 无（内联样式） |
| 国际化 | 1719 个 i18n 文件 | 无 |
| 依赖 | 完整的 React 生态 | React + react-router-dom |
| 构建工具 | Vite | Vite |
| 样式方案 | SCSS Modules | Inline styles / CSSProperties |

---

## 2. 功能页面对比

| 功能 | Logto Console | Aspectus Console |
|------|-------------|-----------------|
| **Dashboard** | 数据概览、图表、快捷入口 | 3 个快捷卡片 |
| **Tenant 管理** | TenantSettings (订阅/域名/计费) | Tenants (创建 + 名称) |
| **用户管理** | Users 列表 + UserDetails (5 tabs) | Users (列表 + 创建 + suspend) |
| UserDetails tabs | Settings + Roles + Organizations + Sessions + Identities | — (单页) |
| 用户设置 | 密码重置、MFA、PAT、社交连接、通行密钥 | — |
| **角色管理** | Roles 列表 + RoleDetails (4 tabs) | Roles (列表 + 分配) |
| RoleDetails tabs | Permissions + Users + Applications + Settings | — |
| **API Key / PAT** | PersonalAccessTokens (创建/编辑 modal) | ApiKeys (列表 + 创建 + 吊销) |
| **应用管理** | Applications + ApplicationDetails (8K+ LOC) | — |
| ApplicationDetails | Settings + SAML + Permissions + Guide | — |
| **API 资源** | ApiResources + ApiResourceDetails | — |
| **组织** | Organizations + OrganizationDetails + Template | ❌ Aspectus 无此概念 |
| **审计日志** | AuditLogs (筛选/搜索/分页) | SQL 查询示例 |
| **登录体验** | SignInExperience (可视化 builder, 9K LOC) | — |
| **连接器** | Connectors (30+ social/SAML/SSO) | ❌ 非目标 |
| **Webhooks** | Webhooks + WebhookDetails | — |
| **MFA** | MFA 配置 | ❌ 非 MVP |
| **安全** | 黑名单、验证码、密码策略 | — |
| **个人设置** | Profile | — |

---

## 3. 架构模式对比

### Logto Console 模式

```
pages/{PageName}/
├── index.tsx              # 主页面
├── index.module.scss      # 样式
├── types.ts               # 类型定义
├── utils.ts               # 工具函数
└── components/            # 页面内子组件（1-10 个）
    └── {SubFeature}/
        ├── index.tsx
        └── index.module.scss
```

**关键模式**：
- 每个页面是**独立的文件夹**，包含自己的样式、类型、子组件
- 复杂页面（如 UserDetails）拆分为 **4-5 个 Tab**，每个 Tab 再拆分子组件
- 全局共享 `components/` 和 `containers/`（Container/Presenter 分离）
- 自建 `ds-components/`（Button、Modal、Table、Form、Dropdown 等 50+ 组件）
- 统一 `hooks/` 层（use-api、use-swr、use-confirm-modal 等）
- SCSS Modules 隔离样式

### Aspectus Console 模式

```
pages/{PageName}.tsx       # 单文件，含样式和逻辑
api/client.ts              # 统一 API 调用
```

**当前特点**：
- **极简**：每个页面一个 `.tsx` 文件，无子组件拆分
- 内联样式（`React.CSSProperties`），无外部样式文件
- 无设计系统组件——直接使用原生 `<input>`、`<button>`、`<table>`
- 无状态管理库——`useState` 直接管理
- 无国际化

---

## 4. 差距分析

### Logto Console 有而 Aspectus Console 缺的

| 能力 | 重要性 | 工作量估计 |
|------|:--:|:--:|
| **表格组件**（排序/分页/筛选） | 🔴 高 | 2-3d |
| **表单校验**（email 格式、必填项） | 🔴 高 | 1d |
| **Modal 弹窗**（确认删除、创建表单） | 🟡 中 | 1d |
| **Toast 通知**（操作成功/失败） | 🟡 中 | 0.5d |
| **错误处理**（API 错误友好展示） | 🟡 中 | 1d |
| **加载状态**（spinner/skeleton） | 🟡 中 | 0.5d |
| **设计系统基础组件**（Button/Input/Table） | 🟡 中 | 2-3d |
| **国际化 (i18n)** | 🟢 低 | 3-5d |
| **深色模式** | 🟢 低 | 1-2d |
| **响应式布局** | 🟢 低 | 1d |

### Aspectus Console 已有的差异化优势

| 优势 | 说明 |
|------|------|
| **API Key 原文一次性展示** | 创建 Key 后在黄色警告框中显示原文——安全 UX 设计 |
| **轻量部署** | 241KB JS bundle，无额外 CSS 框架依赖 |
| **与 Aspectus API 深度耦合** | `client.ts` 直接映射所有端点，无需中间层 |
| **暗色侧边栏** | 已实现基本导航布局 |

---

## 5. 参考 Logto Console 的改进路线

### Phase 1 — 基础可用（当前到 v0.2）

- [ ] 表格组件（分页 + 排序）
- [ ] API 错误 Toast 通知
- [ ] 创建/删除确认 Modal
- [ ] 加载 spinner
- [ ] 表单输入校验

### Phase 2 — 体验提升（v0.3）

- [ ] 设计系统基础组件（Button、Input、Table、Modal、Badge）
- [ ] 审计日志页面（真实数据查询 + 筛选）
- [ ] User 详情页（Settings + Roles 两个 Tab）
- [ ] 环境变量配置（API base URL + Service Token）

### Phase 3 — 完整管理台（v0.5）

- [ ] OAuth2 Client 管理页面（创建/列表/redirect_uri 配置）
- [ ] Service Account 管理页面
- [ ] 配额配置页面
- [ ] 国际化（中/英）
- [ ] 深色模式

---

## 6. 关键借鉴点

从 Logto Console 学到的最有价值的模式：

1. **Tab-based 详情页**：UserDetails = Settings / Roles / Sessions / Org 四个 Tab，避免单页信息过载
2. **页面内 components/ 目录**：每个页面的子组件放在自己的 `components/` 下，而不是全局共享——保持内聚
3. **ConfirmModal 模式**：删除/吊销等危险操作统一用 `DeleteConfirmModal` 确认
4. **ds-components 自建**：不依赖第三方 UI 库，自建 50+ 组件——但这对 Aspectus 来说过度设计
5. **hooks 层分离数据逻辑**：`use-api` 封装 API 调用 + 加载/错误状态
