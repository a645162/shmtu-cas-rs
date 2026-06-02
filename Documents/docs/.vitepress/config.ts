import { defineConfig } from 'vitepress'

function resolveBase() {
  const repo = process.env.GITHUB_REPOSITORY?.split('/')[1]
  if (!process.env.GITHUB_ACTIONS || !repo) {
    return '/'
  }
  return repo.endsWith('.github.io') ? '/' : `/${repo}/`
}

export default defineConfig({
  base: resolveBase(),
  lang: 'zh-CN',
  title: 'shmtu-cas-rs 开发者文档',
  description: '上海海事大学 CAS / OCR Rust 库开发文档',
  cleanUrls: true,
  lastUpdated: true,
  themeConfig: {
    nav: [
      { text: '快速开始', link: '/getting-started' },
      { text: '架构设计', link: '/architecture' },
      { text: 'API 设计', link: '/shmtu-cas/api-overview' },
    ],
    sidebar: [
      {
        text: '概览',
        items: [
          { text: '文档首页', link: '/' },
          { text: '快速开始', link: '/getting-started' },
          { text: '整体架构', link: '/architecture' },
          { text: '工作区与 Crate', link: '/crates' },
        ],
      },
      {
        text: 'shmtu-cas',
        items: [
          { text: 'API 总览', link: '/shmtu-cas/api-overview' },
          { text: 'CAS 与登录流程', link: '/shmtu-cas/cas-and-login' },
          { text: '验证码抽象', link: '/shmtu-cas/captcha' },
          { text: '同步设计', link: '/shmtu-cas/sync' },
          { text: '解析器与数据模型', link: '/shmtu-cas/parser-and-data' },
        ],
      },
      {
        text: 'OCR 组件',
        items: [
          { text: 'shmtu-ocr', link: '/shmtu-ocr/onnx' },
          { text: 'shmtu-ocr-server', link: '/shmtu-ocr-server/service' },
        ],
      },
      {
        text: '集成',
        items: [{ text: '接入示例', link: '/examples/integration' }],
      },
    ],
    outline: [2, 3],
    search: {
      provider: 'local',
    },
    footer: {
      message: 'shmtu-cas-rs Developer Docs',
      copyright: 'Copyright © shmtu-cas-rs',
    },
  },
})
