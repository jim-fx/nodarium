import { defineEnvVars } from '@sveltejs/kit/env';

export const variables = defineEnvVars({
  PUBLIC_ANALYTIC_SCRIPT: { public: true, static: true },
  PUBLIC_CHANNEL: { public: true, static: true }
});
