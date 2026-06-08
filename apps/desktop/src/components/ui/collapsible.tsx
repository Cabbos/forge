"use client"

import * as React from "react"
import { Collapsible as CollapsiblePrimitive } from "@base-ui/react/collapsible"

const Collapsible = React.forwardRef<
  React.ElementRef<typeof CollapsiblePrimitive.Root>,
  CollapsiblePrimitive.Root.Props
>(function Collapsible(props, ref) {
  return <CollapsiblePrimitive.Root ref={ref} data-slot="collapsible" {...props} />
})

Collapsible.displayName = "Collapsible"

const CollapsibleTrigger = React.forwardRef<
  React.ElementRef<typeof CollapsiblePrimitive.Trigger>,
  CollapsiblePrimitive.Trigger.Props
>(function CollapsibleTrigger(props, ref) {
  return (
    <CollapsiblePrimitive.Trigger ref={ref} data-slot="collapsible-trigger" {...props} />
  )
})

CollapsibleTrigger.displayName = "CollapsibleTrigger"

const CollapsibleContent = React.forwardRef<
  React.ElementRef<typeof CollapsiblePrimitive.Panel>,
  CollapsiblePrimitive.Panel.Props
>(function CollapsibleContent(props, ref) {
  return (
    <CollapsiblePrimitive.Panel ref={ref} data-slot="collapsible-content" {...props} />
  )
})

CollapsibleContent.displayName = "CollapsibleContent"

export { Collapsible, CollapsibleTrigger, CollapsibleContent }
