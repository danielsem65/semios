import { readFileSync, writeFileSync } from 'node:fs'

const target = process.argv[2] ?? 'src-tauri/gen/android/app/build.gradle.kts'
const MARKER = '// semios-release-signing'

let source = readFileSync(target, 'utf8')

if (source.includes(MARKER)) {
  console.log(`already patched: ${target}`)
  process.exit(0)
}

if (!/^import java\.io\.FileInputStream$/m.test(source)) {
  const imports = [...source.matchAll(/^import .*$/gm)]
  if (imports.length === 0) {
    throw new Error(`no import statements found in ${target}`)
  }
  const last = imports[imports.length - 1]
  const at = last.index + last[0].length
  source = `${source.slice(0, at)}\nimport java.io.FileInputStream${source.slice(at)}`
}

const buildTypes = /^([ \t]*)buildTypes[ \t]*\{[ \t]*$/m
const bt = source.match(buildTypes)
if (!bt) {
  throw new Error(`could not find the buildTypes block in ${target}`)
}
const outer = bt[1]
const inner = `${outer}    `

const signingBlock = [
  `${MARKER}`,
  `${outer}signingConfigs {`,
  `${inner}create("release") {`,
  `${inner}    val keystorePropertiesFile = rootProject.file("keystore.properties")`,
  `${inner}    val keystoreProperties = Properties()`,
  `${inner}    if (keystorePropertiesFile.exists()) {`,
  `${inner}        keystoreProperties.load(FileInputStream(keystorePropertiesFile))`,
  `${inner}    }`,
  `${inner}    keyAlias = keystoreProperties.getProperty("keyAlias")`,
  `${inner}    keyPassword = keystoreProperties.getProperty("password")`,
  `${inner}    storeFile = file(keystoreProperties.getProperty("storeFile"))`,
  `${inner}    storePassword = keystoreProperties.getProperty("password")`,
  `${inner}}`,
  `${outer}}`,
  '',
].join('\n')

source = `${source.slice(0, bt.index)}${signingBlock}${source.slice(bt.index)}`

const releaseType = /^([ \t]*)getByName\("release"\)[ \t]*\{[ \t]*$/m
const rt = source.match(releaseType)
if (!rt) {
  throw new Error(`could not find the release buildType in ${target}`)
}
const at = rt.index + rt[0].length
source = `${source.slice(0, at)}\n${rt[1]}    signingConfig = signingConfigs.getByName("release")${source.slice(at)}`

writeFileSync(target, source)
console.log(`patched ${target} for release signing`)
