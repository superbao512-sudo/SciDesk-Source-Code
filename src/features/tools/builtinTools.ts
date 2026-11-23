import {projectCommand} from '../projects/projectService';
import {ChartLine,BookOpenText,Calculator,NotebookPen,ChartNoAxesCombined,Scan,type LucideIcon} from 'lucide-react';
import {screenshotCommand,type CaptureSettings} from './screenshot/service';
export type BuiltinTool={id:string;name:string;description:string;icon:LucideIcon;settings?:'screenshot';launch:(openRoute:(path:string)=>void)=>Promise<void>};
// Built-in tools share the app grid; future additions need only one registered entry.
export const builtinTools:BuiltinTool[]=[
 {id:'advanced-screenshot',name:'高级截图',description:'区域截取、标注与悬浮贴图',icon:Scan,settings:'screenshot',launch:async()=>{const {settings}=await screenshotCommand<{settings:CaptureSettings}>('settings');await screenshotCommand('start',{delay:settings.delay})}},
 {id:'wannier-bands-dos',name:'Wannier 能带与 DOS 分析',description:'分析 Wannier 能带、态密度与轨道贡献',icon:ChartLine,launch:async()=>{await projectCommand('tools_open_awbf')}},
 {id:'quick-plot',name:'科研数据快速绘图',description:'选择数据列、叠加曲线和导出图像',icon:ChartLine,launch:async open=>open('/tools/quick-plot')},
 {id:'pdf-reader',name:'PDF 阅读与摘录',description:'选取原文、框选插图与本地批注',icon:BookOpenText,launch:async open=>open('/tools/pdf-reader')},
 {id:'units',name:'物理单位与公式速查',description:'频率、能量和压力换算 · Alt+U',icon:Calculator,launch:async open=>open('/tools/units')},
 {id:'manual-editor',name:'教学手册编辑器',description:'Markdown 写作、公式插图与整本导出',icon:NotebookPen,launch:async openRoute=>{openRoute('/tools/manual-editor')}},
 {id:'plot-digitizer',name:'曲线数据提取',description:'图表标定与真实数值提取',icon:ChartNoAxesCombined,launch:async openRoute=>{openRoute('/tools/plot-digitizer')}},
];
