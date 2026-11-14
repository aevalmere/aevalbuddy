// Prints the environment line that makes transcribe-cpp-sys build
// transcribe.cpp with src-tauri/src/voice/whisper.cmake (the static C++
// runtime, AVX2, no AVX-512), and appends it to $GITHUB_ENV when a workflow
// runs it, so every later cargo step sees it. cargo's [env] table cannot write
// this value: it is an absolute path inside a -D argument, and CMake wants
// forward slashes. src-tauri/build.rs stops a build that lacks the variable.
//
//   node tools/voice-cmake-env.mjs
import { appendFileSync, existsSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const cmake = resolve(here, '..', 'src-tauri', 'src', 'voice', 'whisper.cmake');
if (!existsSync(cmake)) {
  console.error(`missing ${cmake}`);
  process.exit(1);
}
const path = cmake.split('\\').join('/');
if (path.includes(' ')) {
  // The build script splits the value on spaces, and quoting does not survive
  // every shell on the way there.
  console.error(`the path to whisper.cmake has a space in it: ${path}`);
  process.exit(1);
}
const line = `TRANSCRIBE_CMAKE_ARGS=-DCMAKE_PROJECT_INCLUDE=${path}`;
if (process.env.GITHUB_ENV) appendFileSync(process.env.GITHUB_ENV, `${line}\n`);
console.log(line);
