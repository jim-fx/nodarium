#!/usr/bin/env node
import { main } from './cli/main';

// `nodarium nodes | head` closes the pipe before everything is written
process.stdout.on('error', (error: NodeJS.ErrnoException) => {
  if (error.code !== 'EPIPE') throw error;
  process.exit(process.exitCode ?? 0);
});

process.exitCode = await main(process.argv.slice(2));
