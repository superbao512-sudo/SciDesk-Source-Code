// English headwords and Chinese equivalents for reading condensed-matter papers.
const sections:Record<string,string>={
 '晶格与对称性':`crystal lattice|晶格
reciprocal lattice|倒格子
primitive cell|原胞
unit cell|晶胞
supercell|超胞
Bravais lattice|布拉菲格子
reciprocal lattice vector|倒格矢
lattice constant|晶格常数
Brillouin zone|布里渊区
first Brillouin zone|第一布里渊区
zone center|布里渊区中心
point group|点群
space group|空间群
inversion symmetry|空间反演对称性
time-reversal symmetry|时间反演对称性
symmetry breaking|对称性破缺
structural phase transition|结构相变
order parameter|序参量`,
 '声子与输运':`phonon|声子
acoustic phonon|声学声子
optical phonon|光学声子
chiral phonon|手性声子
soft mode|软模
phonon softening|声子软化
phonon dispersion|声子色散
phonon linewidth|声子线宽
phonon lifetime|声子寿命
group velocity|群速度
phase velocity|相速度
mean free path|平均自由程
relaxation time|弛豫时间
scattering rate|散射率
phonon scattering|声子散射
three-phonon scattering|三声子散射
four-phonon scattering|四声子散射
Umklapp scattering|倒逆散射
normal process|正规过程
anharmonicity|非谐性
interatomic force constants|原子间力常数
harmonic approximation|谐近似
quasiharmonic approximation|准谐近似
Grüneisen parameter|格林艾森参数
dynamical matrix|动力学矩阵
imaginary frequency|虚频
lattice thermal conductivity|晶格热导率
electronic thermal conductivity|电子热导率
thermal conductivity|热导率
thermal conductance|热导
thermal diffusivity|热扩散率
thermal Hall conductivity|热霍尔导率
thermal expansion coefficient|热膨胀系数
Boltzmann transport equation|玻尔兹曼输运方程
relaxation time approximation|弛豫时间近似
specific heat|比热容
heat capacity|热容`,
 '电子与能带':`electronic band structure|电子能带结构
band gap|带隙
direct band gap|直接带隙
indirect band gap|间接带隙
band inversion|能带反转
valence band maximum|价带顶
conduction band minimum|导带底
Fermi level|费米能级
Fermi energy|费米能
Fermi surface|费米面
density of states|态密度
projected density of states|投影态密度
effective mass|有效质量
charge carrier|载流子
carrier concentration|载流子浓度
carrier mobility|载流子迁移率
electron-phonon coupling|电子–声子耦合
electron-phonon interaction|电子–声子相互作用
spin-orbit coupling|自旋–轨道耦合
Berry curvature|贝里曲率
Berry phase|贝里相位
Weyl point|外尔点
Dirac cone|狄拉克锥
topological insulator|拓扑绝缘体
electrical conductivity|电导率
Seebeck coefficient|塞贝克系数`,
 '磁性与超导':`magnetic moment|磁矩
magnetic susceptibility|磁化率
ferromagnetism|铁磁性
antiferromagnetism|反铁磁性
exchange interaction|交换相互作用
spin wave|自旋波
magnon|磁振子
Curie temperature|居里温度
Néel temperature|奈尔温度
superconducting gap|超导能隙
critical temperature|临界温度
Cooper pair|库珀对
charge density wave|电荷密度波
spin density wave|自旋密度波`,
 '第一性原理与力学':`density functional theory|密度泛函理论
density functional perturbation theory|密度泛函微扰理论
exchange-correlation functional|交换关联泛函
plane-wave cutoff energy|平面波截断能
pseudopotential|赝势
molecular dynamics|分子动力学
self-consistent field|自洽场
finite displacement method|有限位移法
hydrostatic pressure|静水压力
uniaxial strain|单轴应变
shear strain|剪切应变
bulk modulus|体积模量
shear modulus|剪切模量
elastic constants|弹性常数
equation of state|物态方程
formation enthalpy|形成焓
zero-point energy|零点能`,
};
const normalize=(s:string)=>s.normalize('NFKC').toLowerCase().replace(/[‐‑–—-]/g,' ').replace(/\s+/g,' ').trim();
export const glossary=Object.entries(sections).flatMap(([category,rows])=>rows.split('\n').map(row=>{
 const [en,zh]=row.split('|');return {en,zh,category,key:normalize(en)};
})).sort((a,b)=>b.key.length-a.key.length);
const aliases:Record<string,string>={
 'DFT':'density functional theory','DFPT':'density functional perturbation theory','BTE':'Boltzmann transport equation',
 'RTA':'relaxation time approximation','EPC':'electron-phonon coupling','SOC':'spin-orbit coupling','DOS':'density of states',
 'PDOS':'projected density of states','VBM':'valence band maximum','CBM':'conduction band minimum','BZ':'Brillouin zone',
 'CDW':'charge density wave','SDW':'spin density wave','MFP':'mean free path',
};
export const lookup=(s:string)=>glossary.find(e=>e.key===normalize(aliases[s.trim()]??s));
export function termsIn(s:string){
 const text=' '+normalize(s).replace(/[^\p{L}\p{N} ]/gu,' ')+' ';
 const expanded=new Set((s.match(/\b[A-Z]{2,5}\b/g)??[]).map(a=>normalize(aliases[a]??'')));
 return glossary.filter(e=>expanded.has(e.key)||text.includes(' '+e.key+' ')||text.includes(' '+e.key+'s '));
}
export function searchTerms(s:string){
 const key=normalize(aliases[s.trim().toUpperCase()]??s);return glossary.filter(e=>e.key.includes(key)||e.zh.includes(key)||e.category.includes(key));
}
// Only translate complete, unambiguous relations. Other sentences go to the neural model.
export function physicsPhrase(raw:string):string|null{
 const text=raw.trim().replace(/\s+/g,' ');
 const exact=lookup(text.replace(/[.!]$/,''));if(exact)return exact.zh;
 const amount=text.match(/^(?:the )?(.+?) (increases|decreases|rises|falls) (by|to|from) ([-+]?\d+(?:\.\d+)?\s*(?:%|K|GPa|MPa|THz|meV|eV))(?: to ([-+]?\d+(?:\.\d+)?\s*(?:%|K|GPa|MPa|THz|meV|eV)))?(?: (\[[\d ,–-]+\]))?[.]?$/i);
 if(amount){
  const [,noun,verb,prep,start,end,cite]=amount;
  const subject=lookup(noun);const relation=prep.toLowerCase();
  if(subject&&((relation==='from')===Boolean(end))){
   const trend=/increases|rises/i.test(verb)?'升高':'降低';
   return subject.zh+(end?'从 '+start+' '+trend+'至 '+end:trend+(relation==='to'?'至 ':'了 ')+start)+(cite||'')+'。';
  }
 }
 const match=text.match(/^(?:the )?(.+?) (increases|decreases|rises|falls) with (increasing|decreasing) (temperature|pressure)(?: (above|below) ([-+]?\d+(?:\.\d+)?) (K|GPa|MPa))?(?: (\[[\d ,–-]+\]))?[.]?$/i);
 if(!match)return null;
 const subject=lookup(match[1]);if(!subject)return null;
 const [, ,verb,direction,variable,threshold,n,unit,citation]=match;
 const parameter=variable.toLowerCase()==='temperature'?'温度':'压力';
 if(unit&&((unit.toUpperCase()==='K')!==(parameter==='温度')))return null;
 const condition=threshold?'当'+parameter+(threshold.toLowerCase()==='above'?'高于':'低于')+n+' '+unit+'时，':'';
 return condition+subject.zh+'随'+parameter+(direction.toLowerCase()==='increasing'?'升高':'降低')+'而'+(/increases|rises/i.test(verb)?'升高':'降低')+(citation||'')+'。';
}
