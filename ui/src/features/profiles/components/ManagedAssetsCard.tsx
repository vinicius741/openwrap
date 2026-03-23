import type { ManagedAsset } from '../../../types/ipc'

interface ManagedAssetsCardProps {
  assets: ManagedAsset[]
}

export function ManagedAssetsCard({ assets }: ManagedAssetsCardProps) {
  return (
    <article className="detail-card">
      <h3>Managed assets</h3>
      <ul className="asset-list">
        {assets.map((asset) => (
          <li key={asset.id}>
            <strong>{asset.kind}</strong>
            <span>{asset.relative_path}</span>
          </li>
        ))}
      </ul>
    </article>
  )
}
