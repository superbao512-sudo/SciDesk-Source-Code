import type {Resource} from './learningService';

export const topics = ['VASP', 'Quantum ESPRESSO', '声子', '热输运'] as const;
export type Topic = typeof topics[number];
export const purposes = ['官方文档', '入门教程', '计算实例', '其他参考'] as const;
type Purpose = typeof purposes[number];

type TopicRule = {direct: RegExp; category: RegExp};
const topicRules: Record<Topic, TopicRule> = {
 VASP: {direct: /vasp|\b(?:INCAR|CHGCAR|POSCAR|OUTCAR|POTCAR)\b/i, category: /vasp/i},
 'Quantum ESPRESSO': {direct: /quantum[\s_-]*espresso|pwscf|\bQE\b|\b(?:pw|ph|matdyn)\.x\b|\bEPW\b/i, category: /quantum[\s_-]*espresso|\bQE\b/i},
 '声子': {direct: /声子|振动模|力常数|电声耦合|phonon|phonopy|phono3py|hiphive|alamode|\btdep\b|\bEPW\b|perturbo/i, category: /声子计算|电子-声子耦合/},
 '热输运': {direct: /热输运|热传导|热导率|声子输运|热电|导热|shengbte|giftbte|fourphonon|phono3py|boltztrap|thermal[\s_-]*(conduct|transport)/i, category: /热导率|热输运|热电性质/},
};

// Broad parent categories combine different fields. Use the specific child or
// the bookmark itself, so a VASP setup guide does not inherit a phonon topic.
export function resourceTopics(r: Resource): Topic[] {
 let address = '';
 try {const url = new URL(r.url); address = url.hostname + url.pathname} catch { /* Legacy bookmark. */ }
 const direct = [r.title, r.tags, address].join(' ');
 const category = r.category2 || r.category1;
 return topics.filter(topic => topicRules[topic].direct.test(direct)
  || (r.category2 ? topicRules[topic].category.test(category) : category.trim().toLowerCase() === topic.toLowerCase()));
}

const documentationSites = new Set([
 'vaspkit.com','phonopy.github.io','hiphive.materialsmodeling.org','docs.abinit.org',
 'alamode.readthedocs.io','bte.sjtu.edu.cn','perturbo-code.github.io','docs.epw-code.org',
 'shengbte.org','quantum-espresso.org',
]);

export function resourcePurpose(r: Resource): Purpose {
 let host = '', path = '';
 try {const url = new URL(r.url); host = url.hostname.replace(/^www\./,''); path = url.pathname.toLowerCase()} catch { /* Use the title alone. */ }
 const title = r.title;
 if (/报错|问答|数值过大|error\b|论坛|forum\b|project news|理论小结|基本原理/i.test(title)) return '其他参考';
 if (/实例|案例|为例|举例|\bex\d+|superconducting magnesium diboride/i.test(title)
  || /\/(?:examples?|software-examples|ex\d+|tdep-tutorials)(?:\/|$)/.test(path)) return '计算实例';
 if (/安装|编译|入门|初学|基础教学|实践详解|summer school|getting started|installation|教程|learn vasp the hard way/i.test(title)
  || /\/tutorials?\//.test(path)) return '入门教程';
 if (documentationSites.has(host) || (host === 'vasp.at' && /^\/wiki(?:\/|$)/.test(path))) return '官方文档';
 if (/计算.{0,12}(声子谱|能带|热电优值|热学性质|载流子迁移率|杨氏模量|能量|电导率|电荷|过渡态|轨道成分|方程)|(?:声子谱?|功函数|差分电荷密度|部分电荷密度|热电材料性质).{0,4}计算|校正吸附|supercells for|画声子谱|谱使用.{0,15}矫正|gamma点声子振动模式|状态方程拟合|输出电声耦合矩阵元|做电荷密度差图/i.test(title)) return '计算实例';
 if (/输入文件.{0,5}(浅析|中)|如何显示振动模|简单介绍|教程|基础\d|赝势的选择和使用|实践详解|hands.on/i.test(title)) return '入门教程';
 return '其他参考';
}

// Put the existing handbook, introductory guide and worked example near the
// top of each section. These shortcuts only rank bookmarks in the user's library.
const startingPoints: Record<Topic, string[]> = {
 VASP: ['vasp.at/wiki/index.php/The_VASP_Manual', 'bigbrosci.com', 'bigbrosci.com/2018/10/09/ex61'],
 'Quantum ESPRESSO': ['quantum-espresso.org/documentation/input-data-description', 'zhuanlan.zhihu.com/p/84789491', 'blog.csdn.net/lielie12138/article/details/123331417'],
 '声子': ['phonopy.github.io/phonopy', 'blog.sciencenet.cn/blog-3222255-1068177.html', 'blog.sciencenet.cn/blog-567091-677384.html'],
 '热输运': ['shengbte.org', 'zhuanlan.zhihu.com/p/525939702', 'blog.csdn.net/weixin_44873868/article/details/135567239'],
};

function bookmarkKey(address: string) {
 try {const url = new URL(address); return url.hostname.replace(/^www\./, '') + url.pathname.replace(/\/$/, '')}
 catch {return address}
}

export function topicIndex(rows: Resource[]) {
 const index = new Map<Topic, Resource[]>(topics.map(topic => [topic, []]));
 for (const row of rows) for (const topic of resourceTopics(row)) index.get(topic)!.push(row);
 for (const [topic, entries] of index) {
  const priority = new Map(startingPoints[topic].map((key, i) => [key, i]));
  const rank = (r: Resource) => priority.get(bookmarkKey(r.url)) ?? priority.size;
  entries.sort((a,b) => rank(a) - rank(b));
 }
 return index;
}
