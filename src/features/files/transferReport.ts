export type TransferReport={items:{source:string;destination:string;status:string;message:string}[];cancelled:boolean;scan_warning?:string};
export function transferSummary(report:TransferReport,mode:'copy'|'move'|'link'){
 const done=report.items.filter(item=>item.status==='completed').length;
 const skipped=report.items.filter(item=>item.status==='skipped').length;
 const failed=report.items.filter(item=>!['completed','skipped'].includes(item.status));
 return `已${mode==='link'?'关联':mode==='move'?'移动':'复制'} ${done} 项${skipped?`，跳过 ${skipped} 项`:''}${failed.length?`，未完成 ${failed.length} 项：${failed.map(item=>item.message).join('；')}`:''}${report.scan_warning?`；索引刷新失败：${report.scan_warning}`:''}`;
}
