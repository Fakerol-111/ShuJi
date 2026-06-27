import { sendMessage, setDocumentStatus } from '../api';

/** 准奏文档并通知 pipeline / 内阁继续（与 DocPreview 一致）。 */
export async function approveDocumentAndResume(docId: string, comment?: string): Promise<void> {
  await setDocumentStatus(docId, 'approved', comment || undefined);
  const msg = comment ? `朕已御批。 ${comment}` : '朕已御批。';
  await sendMessage(msg);
}
