import {spawnSync} from 'node:child_process';
import {resolve,join} from 'node:path';
const root=resolve(import.meta.dirname,'..');
const env={...process.env,CARGO_TARGET_DIR:join(root,'development/tauri-target'),CARGO_BUILD_JOBS:process.env.CARGO_BUILD_JOBS||'4'};
const mode=process.argv[2];
const cargoMode=['check','test','project-bridge','ideas-bridge','journals-bridge'].includes(mode);
const command=cargoMode?'cargo':process.execPath;
const args=cargoMode?[...(mode==='project-bridge'?['build','--features','dev-bridges','--bin','project_bridge']:mode==='ideas-bridge'?['build','--features','dev-bridges','--bin','ideas_dev']:mode==='journals-bridge'?['build','--features','dev-bridges','--bin','journal_dev']:[mode]),'--locked','--manifest-path','src-tauri/Cargo.toml']:[join(root,'node_modules/@tauri-apps/cli/tauri.js'),'dev',...process.argv.slice(3)];
const result=spawnSync(command,args,{cwd:root,env,stdio:'inherit'});if(result.error)console.error(result.error.message);process.exit(result.status??1);
