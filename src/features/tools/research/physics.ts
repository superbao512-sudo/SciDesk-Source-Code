const h=6.62607015e-34, e=1.602176634e-19, c=299792458, k=1.380649e-23;
export type Measure={label:string;tex:string;factor:number};
export const groups:{label:string;formula:string;units:Measure[]}[]=[
 {label:'频率、波数与能量',formula:String.raw`E=h\nu=\hbar\omega=hc\tilde{\nu}=k_{\mathrm B}T,\quad\omega=2\pi\nu`,units:[
  {label:'THz（频率 ν）',tex:'\\mathrm{THz}',factor:h*1e12},
  {label:'cm⁻¹（波数）',tex:'\\mathrm{cm}^{-1}',factor:h*c*100},
  {label:'meV',tex:'\\mathrm{meV}',factor:e*1e-3},
  {label:'rad/ps（角频率 ω）',tex:'\\mathrm{rad\\,ps}^{-1}',factor:h*1e12/(2*Math.PI)},
  {label:'eV',tex:'\\mathrm{eV}',factor:e},{label:'K（等效温度）',tex:'\\mathrm K',factor:k}]},
 {label:'压力与能量密度',formula:String.raw`1\,\mathrm{GPa}=10\,\mathrm{kbar},\quad 1\,\mathrm{eV}/\mathrm{\mathring A}^{3}=160.2176634\,\mathrm{GPa}`,units:[
  {label:'GPa',tex:'\\mathrm{GPa}',factor:1e9},{label:'kbar',tex:'\\mathrm{kbar}',factor:1e8},
  {label:'eV/Å³',tex:'\\mathrm{eV}/\\mathrm{\\mathring A}^{3}',factor:e*1e30},{label:'Pa',tex:'\\mathrm{Pa}',factor:1}]}
];
export function convert(input:string,from:Measure,to:Measure){
 const s=input.trim();if(!/^[+-]?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?$/i.test(s))return null;
 const v=Number(s)*(from.factor/to.factor);return Number.isFinite(v)?Number(v.toPrecision(11)):null;
}
