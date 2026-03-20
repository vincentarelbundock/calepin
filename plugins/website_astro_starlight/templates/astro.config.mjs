// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
  integrations: [
    starlight({
      title: '{{title}}',{{logo_config}}{{favicon_config}}{{social_config}}
      customCss: ['./src/styles/calepin.css'],
      sidebar: [
{{sidebar}}
      ],
    }),
  ],
  trailingSlash: 'never',
});
