interface DnsStatusNoticeProps {
  message: string | null
}

export function DnsStatusNotice({ message }: DnsStatusNoticeProps) {
  if (!message) {
    return null
  }

  return (
    <div className="dns-observation">
      <strong>DNS status</strong>
      <p>{message}</p>
    </div>
  )
}
