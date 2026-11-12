/**
 * Tests for turning pasted links into the domains listed for a profile's
 * "only listed domains" DNS mode.
 */

import { describe, it } from 'node:test'
import assert from 'node:assert/strict'

import {
  addDomain,
  findCoveringDomain,
  parseDomainInput,
  removeDomain,
} from '../ui/src/features/profiles/model/dns-domains.ts'

describe('parseDomainInput', () => {
  it('cleans a full URL down to its host and offers the broadest domain first', () => {
    const result = parseDomainInput(
      'https://atc.bmwgroup.net/jira/secure/RapidBoard.jspa?rapidView=74841&projectKey=ORBITCOST',
    )
    assert.deepEqual(result, {
      kind: 'ok',
      host: 'atc.bmwgroup.net',
      options: ['bmwgroup.net', 'atc.bmwgroup.net'],
    })
  })

  it('handles links without a scheme, with ports, credentials and fragments', () => {
    assert.deepEqual(parseDomainInput('  ATC.BMWGroup.net:8443/jira#top ').options, [
      'bmwgroup.net',
      'atc.bmwgroup.net',
    ])
    assert.equal(parseDomainInput('ssh://user:pw@git.example.com:22/repo.git').host, 'git.example.com')
    assert.equal(parseDomainInput('//cdn.example.com/x').host, 'cdn.example.com')
  })

  it('offers every level between the registrable domain and the host', () => {
    assert.deepEqual(parseDomainInput('a.b.c.example.com').options, [
      'example.com',
      'c.example.com',
      'b.c.example.com',
      'a.b.c.example.com',
    ])
  })

  it('never offers a bare public suffix', () => {
    assert.deepEqual(parseDomainInput('https://portal.bmw.com.br/x').options, [
      'bmw.com.br',
      'portal.bmw.com.br',
    ])
    assert.deepEqual(parseDomainInput('bmw.co.uk').options, ['bmw.co.uk'])
    assert.deepEqual(parseDomainInput('bmwgroup.net').options, ['bmwgroup.net'])
  })

  it('accepts wildcards, trailing dots and single-label internal names', () => {
    assert.deepEqual(parseDomainInput('*.bmwgroup.net').options, ['bmwgroup.net'])
    assert.deepEqual(parseDomainInput('bmwgroup.net.').options, ['bmwgroup.net'])
    assert.deepEqual(parseDomainInput('http://intranet/').options, ['intranet'])
  })

  it('reports empty input, IP addresses and junk', () => {
    assert.equal(parseDomainInput('   ').kind, 'empty')
    assert.equal(parseDomainInput('https://10.0.1.50/admin').kind, 'error')
    assert.equal(parseDomainInput('http://[::1]/').kind, 'error')
    assert.equal(parseDomainInput('not a domain').kind, 'error')
    assert.equal(parseDomainInput('bmw_group.net').kind, 'error')
    assert.equal(parseDomainInput("bmw.com'; rm -rf /").kind, 'error')
  })
})

describe('domain list editing', () => {
  it('finds the listed parent that already covers a host', () => {
    assert.equal(findCoveringDomain('atc.bmwgroup.net', ['bmw.com', 'bmwgroup.net']), 'bmwgroup.net')
    assert.equal(findCoveringDomain('notbmwgroup.net', ['bmwgroup.net']), null)
  })

  it('adding a parent replaces the subdomains it covers', () => {
    assert.deepEqual(addDomain(['atc.bmwgroup.net', 'bmw.com', 'git.bmwgroup.net'], 'bmwgroup.net'), [
      'bmw.com',
      'bmwgroup.net',
    ])
  })

  it('adding a covered subdomain changes nothing', () => {
    assert.deepEqual(addDomain(['bmwgroup.net'], 'atc.bmwgroup.net'), ['bmwgroup.net'])
  })

  it('removes exactly one entry', () => {
    assert.deepEqual(removeDomain(['bmw.com', 'bmwgroup.net'], 'bmw.com'), ['bmwgroup.net'])
  })
})
