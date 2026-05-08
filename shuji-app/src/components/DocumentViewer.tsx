import Markdown from "react-markdown";

interface Props {
  title: string;
  content: string;
  onClose?: () => void;
}

export default function DocumentViewer({ title, content, onClose }: Props) {
  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="bg-white rounded-lg shadow-xl w-3/4 max-h-[80vh] flex flex-col">
        <div className="flex items-center justify-between p-4 border-b">
          <h2 className="text-lg font-bold">{title}</h2>
          {onClose && (
            <button
              onClick={onClose}
              className="text-gray-500 hover:text-gray-700 text-2xl leading-none"
            >
              &times;
            </button>
          )}
        </div>
        <div className="p-6 overflow-y-auto prose prose-sm max-w-none">
          <Markdown>{content}</Markdown>
        </div>
      </div>
    </div>
  );
}
