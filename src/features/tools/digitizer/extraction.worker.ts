import {extractCurve} from './extract';
self.onmessage=event=>{try{self.postMessage({ok:true,result:extractCurve(event.data.image,event.data.roi,event.data.options)})}catch(error){self.postMessage({ok:false,error:String(error)})}};
