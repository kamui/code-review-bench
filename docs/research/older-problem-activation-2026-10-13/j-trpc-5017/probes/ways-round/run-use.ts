// Usage: bun run-use.ts <probe.ts>   executes the probe's use() and prints what it returns, or the error it throws.
// Types are not checked here; bun strips them.
const probe = await import(process.argv[2]!);
try {
  console.log('RESULT', JSON.stringify(await probe.use()));
} catch (error) {
  console.log('THREW', error instanceof Error ? `${error.name}: ${error.message.replace(/\s+/g, ' ').slice(0, 300)}` : String(error));
}
