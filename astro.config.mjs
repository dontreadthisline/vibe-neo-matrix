// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import lucode from 'lucode-starlight';

// https://astro.build/config
export default defineConfig({
  srcDir: './site',
  outDir: './docs',
  base: '/vibe-neo-matrix/',
  integrations: [
    starlight({
      title: 'neo-rainst',
      plugins: [lucode()],
      social: [
        {
          icon: 'github',
          label: 'GitHub',
          href: 'https://github.com/dontreadthisline/vibe-neo-matrix',
        },
      ],
      sidebar: [
        {
          label: '指南',
          items: [
            { label: '快速开始', slug: 'getting-started' },
            { label: '配置', slug: 'configuration' },
            { label: 'Claude Code 集成', slug: 'claude-integration' },
          ],
        },
        {
          label: '参考',
          items: [
            { label: 'CLI 参数', slug: 'reference/cli' },
            { label: '字符源', slug: 'reference/charsets' },
            { label: '颜色主题', slug: 'reference/colors' },
          ],
        },
      ],
    }),
  ],
});
