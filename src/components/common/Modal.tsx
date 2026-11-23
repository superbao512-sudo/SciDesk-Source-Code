import {uiText} from '../../locales/language';
import {useEffect,useRef,useId,type ReactNode} from 'react';
import {X} from 'lucide-react';
import {commonLabels} from '../../locales/zh-CN';
export function Modal({ title, children, footer, className, closeDisabled = false, onClose }: { title: string; children: ReactNode; footer?: ReactNode; className?: string; closeDisabled?: boolean; onClose: () => void }) {
  const ref = useRef<HTMLDialogElement>(null);
  const titleId = useId();
  useEffect(() => {
    const dialog = ref.current!;
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    dialog.showModal();
    return () => { dialog.close(); if (opener?.isConnected) opener.focus({preventScroll:true}); };
  }, []);
  const close = () => { if (!closeDisabled) onClose(); };
  return <dialog ref={ref} className={`modal ${className||''}`} aria-labelledby={titleId} aria-busy={closeDisabled || undefined} onCancel={event => { event.preventDefault(); close(); }} onMouseDown={event => { if (event.target !== event.currentTarget) return; const rect = event.currentTarget.getBoundingClientRect(); if (event.clientX < rect.left || event.clientX > rect.right || event.clientY < rect.top || event.clientY > rect.bottom) close(); }}><div className="modal-header"><h2 id={titleId}>{title}</h2><button type="button" className="icon-button" disabled={closeDisabled} onClick={close} aria-label={uiText(commonLabels.close)}><X size={19}/></button></div><div className="modal-content">{children}</div>{footer && <div className="modal-footer">{footer}</div>}</dialog>;
}
