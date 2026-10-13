import { zodResolver } from "@hookform/resolvers/zod";
import { ChevronDown, ChevronUp } from "lucide-react";
import { useEffect, useState } from "react";
import { Controller, useForm } from "react-hook-form";
import { Button } from "~/components/ui/buttons/button.component";
import { Field } from "~/components/ui/forms/field/field.component";
import { MultiSelectInput } from "~/components/ui/forms/input/multi-select-input.component";
import { SelectInput } from "~/components/ui/forms/input/select-input.component";
import { TextInput } from "~/components/ui/forms/input/text-input.component";
import { ToggleInput } from "~/components/ui/forms/input/toggle-input.component";
import { useCreateJobMutation } from "~/hooks/use-create-job-mutation.hook";
import type { Dataset } from "~/schemas/domain/dataset.schema";
import {
	availableFunctionSchema,
	lossFunctionSchema,
} from "~/schemas/domain/enums.schema";
import {
	createJobDtoSchema,
	createJobFormSchema,
} from "~/schemas/dtos/job.dto";
import { capitalize } from "~/utils/capitalize.util";

type AddJobProps = {
	profileId: string;
	datasets: Dataset[];
	onSuccess: () => void;
	onCancel: () => void;
};

export function AddJob({
	profileId,
	datasets,
	onSuccess,
	onCancel,
}: AddJobProps) {
	const [showAdvanced, setShowAdvanced] = useState(false);
	const [apiError, setApiError] = useState<string | null>(null);

	const { mutate: createJob, isPending } = useCreateJobMutation(profileId);

	const {
		register,
		handleSubmit,
		control,
		watch,
		formState: { errors },
	} = useForm({
		resolver: zodResolver(createJobFormSchema),
		defaultValues: {
			name: `Job#${Date.now()}`,
			runsAgainst: datasets.length > 0 ? datasets[0].id : undefined,
			loss: lossFunctionSchema.options[0],
			nonTerminals: ["add", "sub", "mul", "div"],
			generations: 100,
			population: 100,
			simplify: false,
			maxSize: 15,
			numberOfTournaments: 5,
			crossoverProbability: 0.9,
			mutationProbability: 0.3,
			optimizationIterations: 5,
			optimizationRepeats: 3,
			maxParamCount: -1,
			split: 1,
			activeGroupByColumns: [],
			postProcessingType: null,
		},
	});

	useEffect(() => {
		const subscription = watch(() => setApiError(null));
		return () => subscription.unsubscribe();
	}, [watch]);

	const onSubmit = handleSubmit((data) => {
		setApiError(null);

		const parsed = createJobDtoSchema.safeParse(data);
		if (!parsed.success) {
			return;
		}

		createJob([parsed.data], {
			onSuccess: () => {
				onSuccess();
			},
			onError: (err) => {
				setApiError(
					err instanceof Error
						? err.message
						: "Failed to create job. Please try again.",
				);
			},
		});
	});

	const datasetOptions = datasets.map((d) => ({ label: d.name, value: d.id }));

	const lossOptions = lossFunctionSchema.options.map((opt) => ({
		label: opt,
		value: opt,
	}));

	const functionOptions = availableFunctionSchema.options.map((opt) => ({
		label: capitalize(opt),
		value: opt,
	}));

	const selectedDatasetId = watch("runsAgainst");
	const selectedDataset = datasets.find((d) => d.id === selectedDatasetId);
	const selectedDatasetDataArtifact = selectedDataset?.artifacts.find(
		(a) => a.role === "data",
	);

	return (
		<form
			onSubmit={onSubmit}
			className="p-4 border border-border rounded-lg bg-background-surface space-y-4"
		>
			<h3 className="text-md font-medium">New Job</h3>

			<div className="space-y-2">
				<span className="flex items-center gap-2 text-xs text-gray-400 hover:text-gray-200 transition-colors">
					Basic Settings
				</span>

				<div className="space-y-6">
					<div className="space-y-4">
						<div className="grid grid-cols-2 gap-4">
							<Field error={errors.name}>
								<Field.Label required>Name</Field.Label>
								<Field.Control>
									<TextInput
										placeholder="e.g. my-experiment-v1"
										{...register("name")}
									/>
								</Field.Control>
								<Field.Error />
							</Field>

							<Field error={errors.runsAgainst}>
								<Field.Label required>Dataset</Field.Label>
								<Field.Control>
									<SelectInput
										options={
											datasetOptions.length > 0
												? datasetOptions
												: [
														{
															label: "No datasets linked to this profile",
															value: "",
														},
													]
										}
										disabled={datasetOptions.length === 0}
										{...register("runsAgainst")}
									/>
								</Field.Control>
								<Field.Error />
							</Field>
						</div>

						<div className="grid grid-cols-3 gap-4">
							<Field error={errors.loss}>
								<Field.Label>Loss Function</Field.Label>
								<Field.Control>
									<SelectInput options={lossOptions} {...register("loss")} />
								</Field.Control>
								<Field.Error />
							</Field>

							<Field error={errors.generations}>
								<Field.Label>Generations</Field.Label>
								<Field.Control>
									<TextInput
										type="number"
										{...register("generations", { valueAsNumber: true })}
									/>
								</Field.Control>
								<Field.Error />
							</Field>

							<Field error={errors.population}>
								<Field.Label>Population</Field.Label>
								<Field.Control>
									<TextInput
										type="number"
										{...register("population", { valueAsNumber: true })}
									/>
								</Field.Control>
								<Field.Error />
							</Field>
						</div>

						<Controller
							name="nonTerminals"
							control={control}
							render={({ field }) => (
								<Field
									error={
										errors.nonTerminals as
											| import("react-hook-form").FieldError
											| undefined
									}
								>
									<Field.Label>Available Functions</Field.Label>
									<Field.Control>
										<MultiSelectInput
											options={functionOptions}
											value={field.value ?? []}
											onChange={field.onChange}
										/>
									</Field.Control>
									<Field.Error />
								</Field>
							)}
						/>

						<Field>
							<Field.Label>
								<Field.Control className="p-2 space-x-2">
									<div className="flex flex-col">
										<span>Simplify</span>
										<Field.Description>
											Apply algebraic simplification after evolution
										</Field.Description>
									</div>

									<ToggleInput {...register("simplify")} />
								</Field.Control>
							</Field.Label>
						</Field>
					</div>

					{selectedDatasetDataArtifact?.groupByColumns && (
						<div>
							<Controller
								name="activeGroupByColumns"
								control={control}
								render={({ field }) => (
									<Field
										error={
											errors.activeGroupByColumns as
												| import("react-hook-form").FieldError
												| undefined
										}
									>
										<Field.Label>Active Group By Columns</Field.Label>
										<Field.Control>
											<MultiSelectInput
												options={
													selectedDatasetDataArtifact?.groupByColumns?.map(
														(col) => ({
															label: col,
															value: col,
														}),
													) ?? []
												}
												value={field.value ?? []}
												onChange={field.onChange}
											/>
										</Field.Control>
										<Field.Error />
									</Field>
								)}
							/>

							<Controller
								name="postProcessingType"
								control={control}
								render={({ field }) => (
									<Field
										error={
											errors.postProcessingType as
												| import("react-hook-form").FieldError
												| undefined
										}
									>
										<Field.Label>Post Processing Type</Field.Label>
										<Field.Control>
											<SelectInput
												options={[
													{
														label: "None",
														value: "NONE",
													},
													{
														label: "Grouped Softmax",
														value: "GROUPED_SOFTMAX",
													},
												]}
												value={field.value ?? "NONE"}
												onChange={field.onChange}
											/>
										</Field.Control>
										<Field.Error />
									</Field>
								)}
							/>
						</div>
					)}
				</div>
			</div>

			<div className="border-t border-border pt-2">
				<button
					type="button"
					onClick={() => setShowAdvanced((v) => !v)}
					className="flex items-center gap-2 text-xs text-gray-400 hover:text-gray-200 transition-colors"
				>
					{showAdvanced ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
					Advanced Settings
				</button>
			</div>

			{showAdvanced && (
				<div className="grid grid-cols-2 gap-4">
					<Field error={errors.maxSize}>
						<Field.Label>Max Size</Field.Label>
						<Field.Control>
							<TextInput
								type="number"
								{...register("maxSize", { valueAsNumber: true })}
							/>
						</Field.Control>
						<Field.Error />
					</Field>

					<Field error={errors.numberOfTournaments}>
						<Field.Label>Number of Tournaments</Field.Label>
						<Field.Control>
							<TextInput
								type="number"
								{...register("numberOfTournaments", { valueAsNumber: true })}
							/>
						</Field.Control>
						<Field.Error />
					</Field>

					<Field error={errors.crossoverProbability}>
						<Field.Label>Crossover Probability</Field.Label>
						<Field.Control>
							<TextInput
								type="number"
								step="0.01"
								{...register("crossoverProbability", { valueAsNumber: true })}
							/>
						</Field.Control>
						<Field.Error />
					</Field>

					<Field error={errors.mutationProbability}>
						<Field.Label>Mutation Probability</Field.Label>
						<Field.Control>
							<TextInput
								type="number"
								step="0.01"
								{...register("mutationProbability", { valueAsNumber: true })}
							/>
						</Field.Control>
						<Field.Error />
					</Field>

					<Field error={errors.optimizationIterations}>
						<Field.Label>Optimization Iterations</Field.Label>
						<Field.Control>
							<TextInput
								type="number"
								{...register("optimizationIterations", { valueAsNumber: true })}
							/>
						</Field.Control>
						<Field.Error />
					</Field>

					<Field error={errors.optimizationRepeats}>
						<Field.Label>Optimization Repeats</Field.Label>
						<Field.Control>
							<TextInput
								type="number"
								{...register("optimizationRepeats", { valueAsNumber: true })}
							/>
						</Field.Control>
						<Field.Error />
					</Field>

					<Field error={errors.maxParamCount}>
						<Field.Label>Max Param Count</Field.Label>
						<Field.Control>
							<TextInput
								type="number"
								{...register("maxParamCount", { valueAsNumber: true })}
							/>
						</Field.Control>
						<Field.Error />
					</Field>

					<Field error={errors.split}>
						<Field.Label>Split</Field.Label>
						<Field.Control>
							<TextInput
								type="number"
								{...register("split", { valueAsNumber: true })}
							/>
						</Field.Control>
						<Field.Error />
					</Field>
				</div>
			)}

			{apiError && (
				<div className="text-red-500 text-sm font-medium mt-2">{apiError}</div>
			)}

			<Button.Group mode="seamless">
				<Button onClick={onCancel} variant="outline">
					Cancel
				</Button>

				<Button type="submit" disabled={isPending} variant="primary">
					{isPending ? "Creating..." : "Create Job"}
				</Button>
			</Button.Group>
		</form>
	);
}
