export const navigationLabels={home:'首页',projects:'项目',ideas:'想法',literature:'文献',learning:'学习',notes:'笔记',data:'数据',figures:'图表',writing:'写作',ai:'AI',archive:'归档',settings:'设置'};
export const commonLabels={save:'保存',cancel:'取消',close:'关闭',edit:'编辑',remove:'移除',delete:'删除',open:'打开',search:'搜索',create:'新建',refresh:'刷新',retry:'重试',export:'导出',reset:'恢复默认',saved:'已保存'};
export const sectionLabels:Record<string,string>={abstract:'摘要',introduction:'引言',background:'背景',methods:'方法',methodology:'方法学',computational_methods:'计算方法',theory:'理论',results:'结果',discussion:'讨论',results_and_discussion:'结果与讨论',conclusion:'结论',summary:'总结',acknowledgements:'致谢',references:'参考文献',supplementary:'补充材料',other:'其他'};
export const objectLabels:Record<string,string>={project:'项目',idea:'想法',paper:'文献',manual:'学习手册',note:'笔记',task:'任务',dataset:'数据集',calculation:'计算记录',figure:'图表',manuscript:'稿件',section:'章节',file:'文件',paper_text:'文献正文',paper_page:'文献页面',paper_section:'文献章节',literature_corpus:'文献语料库'};
export const statusLabels:Record<string,Record<string,string>>={
 project:{idea:'构思中',active:'进行中',paused:'已暂停',writing:'写作中',submitted:'已投稿',completed:'已完成',archived:'已归档'},
 idea:{inbox:'收件箱',active:'进行中',to_verify:'待验证',validated:'已验证',rejected:'已否定',completed:'已完成'},
 task:{todo:'待办',in_progress:'进行中',completed:'已完成',cancelled:'已取消',open:'待解决',investigating:'研究中',resolved:'已解决'},
 paper:{unread:'未读',reading:'阅读中',read:'已读',reviewed:'已评阅',important:'重要',archived:'已归档'},
 manual:{not_started:'未开始',learning:'学习中',review:'复习中',mastered:'已掌握'},
 dataset:{raw:'原始数据',processed:'处理数据',derived:'派生数据'},
 calculation:{draft:'草稿',imported:'已导入',ready:'就绪',running:'运行中',finished:'已完成',completed:'已完成',failed:'失败',cancelled:'已取消',unknown:'未知',incomplete:'不完整',parsed:'已解析',prepared:'已准备',running_external:'外部运行中'},
 figure:{draft:'草稿',working:'制作中',in_progress:'制作中',review:'待审阅',final:'定稿',archived:'已归档'},
 manuscript:{idea:'构思中',revision:'修订中',draft:'草稿',writing:'写作中',review:'待审阅',submitted:'已投稿',published:'已发表',final:'定稿',archived:'已归档'},
 corpus:{empty:'空语料库',building:'构建中',ready:'就绪',partial:'部分可用',stale:'待刷新',failed:'失败'},
 extraction:{not_extracted:'未提取',extracting:'提取中',extracted:'已提取',failed:'提取失败',missing:'文件缺失',stale:'待重新提取',unsupported:'不支持',image_based_or_no_text:'图像型或无文本'},
 file:{available:'可用',missing:'缺失',permission_denied:'无访问权限',modified:'已修改',unknown:'未知'},
};
export const statusLabel=(domain:string,value:string)=>statusLabels[domain]?.[value]||'未知';
export const languageLabel=(value:string)=>({English:'英语',Chinese:'中文',Other:'其他语言',Unknown:'未知',All:'全部语言'}[value]||'未知');
export const activityLabels:Record<string,string>={'Extracted Paper Text':'提取了文献正文','Refreshed Paper Text':'刷新了文献正文','Built Literature Corpus':'构建了文献语料库','Refreshed Literature Corpus':'刷新了文献语料库','Updated Corpus Settings':'更新了语料库设置',created:'创建了',updated:'更新了',deleted:'删除了',imported:'导入了',archived:'归档了',restored:'恢复了',completed:'完成了',linked:'关联了',unlinked:'取消关联了',opened:'打开了',saved:'保存了'};
export function activityLabel(action:string,type:string,description:string){if(/[\u4e00-\u9fff]/.test(description))return description;if(!activityLabels[action]&&!activityLabels[action.split('.').at(-1)||''])return description;return activityLabels[action]||`${activityLabels[action.split('.').at(-1)||'']||'更新了'}${objectLabels[type]||'记录'}`;}
export function userError(reason:unknown){const text=String(reason);if(/[\u4e00-\u9fff]/.test(text))return text.replace(/^Error:\s*/,'');if(/not found|missing|no such file/i.test(text))return '找不到文件，请检查路径或重新定位。';if(/permission|access denied/i.test(text))return '没有访问权限，请检查文件或目录权限。';if(/invalid|malformed|syntax|parse/i.test(text))return '内容格式无效，请检查文件或输入后重试。';return '操作失败，请检查本地文件与设置后重试。';}
export const fileRoleLabels:Record<string,string>={preview:'预览',source:'来源',export:'导出',script:'脚本',main:'主文件',section:'章节',reference:'引用',input:'输入',output:'输出',log:'日志',config:'配置',other:'其他'};
