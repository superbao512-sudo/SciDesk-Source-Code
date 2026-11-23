import {spawnSync} from 'node:child_process';
import {mkdirSync,writeFileSync,copyFileSync,readFileSync,statSync} from 'node:fs';
import {resolve,join,relative,isAbsolute} from 'node:path';
import {createHash} from 'node:crypto';
const root=resolve(import.meta.dirname,'..');
const version=JSON.parse(readFileSync(join(root,'package.json'),'utf8')).version;
const stamp=new Date().toISOString().replace(/[:.]/g,'-');
const resume=process.argv[2]==='--resume';
if(resume&&!process.argv[3])throw new Error('--resume 需要本次已完成 Release 构建的打包目录');
const output=resume?resolve(process.argv[3]):join(root,'development/packages',stamp);
const inside=relative(join(root,'development/packages'),output);
if(!inside||inside.startsWith('..')||isAbsolute(inside))throw new Error('打包目录必须在 development/packages 内');
if(!resume)mkdirSync(output,{recursive:true});
const seed=join(output,'personal-seed');
const env={...process.env,CARGO_TARGET_DIR:join(root,'development/tauri-target'),CARGO_BUILD_JOBS:process.env.CARGO_BUILD_JOBS||'4'};
function run(command,args){const p=spawnSync(command,args,{cwd:root,env,stdio:'inherit'});if(p.error)throw p.error;if(p.status!==0)process.exit(p.status??1);}
const config=join(output,'personal-bundle.json');
if(!resume){
run('python',[join(root,'scripts/snapshot-personal-data.py'),seed,...(process.argv[2]?['--source',resolve(process.argv[2])]:[])]);
writeFileSync(config,JSON.stringify({productName:'SciDesk 2.0',bundle:{active:true,targets:['nsis'],resources:{[seed]:'personal-seed/','../docs/third-party/':'third-party/'},windows:{webviewInstallMode:{type:'offlineInstaller'},nsis:{installMode:'currentUser',languages:['SimpChinese','English'],displayLanguageSelector:false,installerHooks:join(root,'src-tauri/installer-hooks.nsh')}}}},null,2));
}
run(process.execPath,[join(root,'node_modules/@tauri-apps/cli/tauri.js'),resume?'bundle':'build','--config',config,'--bundles','nsis','--ci']);
const bundle=join(env.CARGO_TARGET_DIR,'release/bundle/nsis');
const installer=join(bundle,`SciDesk 2.0_${version}_x64-setup.exe`);
const final=join(output,`SciDesk_${version}_x64_personal-setup.exe`);copyFileSync(installer,final);
const digest=createHash('sha256').update(readFileSync(final)).digest('hex');
writeFileSync(final+'.sha256',digest+'  '+final.split(/[\\/]/).at(-1)+'\n');
writeFileSync(join(output,'安装说明.txt'),`SciDesk 2.0 ${version} 个人数据安装版\r\n\r\n在 Windows 10/11 x64 安装，可自行选择当前用户可写的安装目录。\r\n安装目录内 scidesk.exe 与 SciDeskData 同级；安装完成即创建 SciDeskData。\r\n首次启动：优先保留同级数据；复制迁移旧 %LOCALAPPDATA%\\com.scidesk.v2.desktop；缺少的模块才恢复内置个人快照。旧目录不删除。\r\nSciDeskData 内：database（管理/学习/手册/工具），journals（期刊/头像），ideas（想法/附件），tools（截图设置），Projects（默认新项目），Exports\\Screenshots、Exports\\PlotDigitizer（默认导出），cache\\webview、logs。\r\n学习、期刊、头像、想法及现有管理记录已内置。新电脑无需 Node、Rust 或原工程，含 WebView2 离线安装组件。\r\n卸载默认保留 SciDeskData；重装到原目录继续使用。升级到另一安装目录时请一并复制原 SciDeskData。\r\n外部项目、手册、学习备份文件只保留关联，需另外搬迁和重新关联；不会擅自移动原科研文件。\r\n如旧版仍在运行或存在未完成操作，请先关闭并恢复；安装到不可写目录会明确报错，不暗中改用其他路径。\r\n安装包未签名，Windows 可能显示未知发布者。本安装包含个人数据，请自行保管。\r\n`,'utf8');
writeFileSync(join(output,'artifact.json'),JSON.stringify({version,installer:final,sha256:digest,bytes:statSync(final).size,seed:JSON.parse(readFileSync(join(seed,'manifest.json'),'utf8')),data_directory:'<installation>/SciDeskData',webview2:'offlineInstaller',signed:false},null,2));
console.log(JSON.stringify({installer:final,sha256:digest,output},null,2));
